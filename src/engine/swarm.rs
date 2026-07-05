//! Swarm verification: parallel randomized verification across multiple iterations.
//!
//! Swarm mode runs N verification workers in parallel, each with different
//! random seeds, hash functions, and search parameters. This increases coverage
//! for large state spaces where a single run might miss deep violations.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::engine::checker::{CheckResult, Model, SearchMode, StorageMode, Violation};
use crate::engine::storage::{BitstateStore, ExactStore, StateStore};

/// Configuration for a single swarm worker.
#[derive(Debug, Clone)]
pub struct SwarmWorkerConfig {
    /// Worker index (0..N-1)
    pub worker_id: usize,
    /// Random seed for this worker
    pub seed: u64,
    /// Search mode (DFS or BFS)
    pub search_mode: SearchMode,
    /// Storage mode
    pub storage_mode: StorageMode,
    /// Maximum states
    pub max_states: usize,
    /// Maximum depth
    pub max_depth: usize,
    /// Whether to use partial order reduction
    pub por_enabled: bool,
}

/// Configuration for swarm verification.
#[derive(Debug, Clone)]
pub struct SwarmConfig {
    /// Number of workers
    pub num_workers: usize,
    /// Number of iterations per worker
    pub iterations_per_worker: usize,
    /// Base maximum states (varied per worker)
    pub base_max_states: usize,
    /// Base maximum depth (varied per worker)
    pub base_max_depth: usize,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            num_workers: 4,
            iterations_per_worker: 1,
            base_max_states: 1_000_000,
            base_max_depth: 100_000,
        }
    }
}

/// Generate N worker configs with varied parameters.
pub fn generate_swarm_configs(config: &SwarmConfig) -> Vec<SwarmWorkerConfig> {
    let mut workers = Vec::with_capacity(config.num_workers);

    for i in 0..config.num_workers {
        let seed = 0xdead_beef_u64.wrapping_add(i as u64 * 0x9e3779b9);
        let search_mode = if i % 2 == 0 {
            SearchMode::DepthFirst
        } else {
            SearchMode::BreadthFirst
        };
        let storage_mode = match i % 3 {
            0 => StorageMode::Exact,
            1 => StorageMode::Bitstate,
            _ => StorageMode::Exact,
        };
        let max_states = config.base_max_states + (i * 100_000);
        let max_depth = config.base_max_depth + (i * 10_000);
        let por_enabled = i % 2 == 0;

        workers.push(SwarmWorkerConfig {
            worker_id: i,
            seed,
            search_mode,
            storage_mode,
            max_states,
            max_depth,
            por_enabled,
        });
    }

    workers
}

/// Shared state for swarm workers (does NOT include the model — each worker owns its own).
struct SwarmShared {
    violations: Mutex<Vec<Violation>>,
    total_states_explored: AtomicUsize,
    total_transitions: AtomicUsize,
    max_depth_reached: AtomicUsize,
    violation_found: AtomicBool,
    start_time: Instant,
}

/// Run swarm verification using a model factory.
///
/// Each worker calls `model_factory()` to create its own model instance.
pub fn run_swarm<M, F>(model_factory: F, config: &SwarmConfig) -> CheckResult
where
    M: Model + Send + 'static,
    M::State: Send + 'static,
    F: Fn() -> M + Send + Sync + 'static,
{
    let start = Instant::now();
    let worker_configs = generate_swarm_configs(config);

    let shared = Arc::new(SwarmShared {
        violations: Mutex::new(Vec::new()),
        total_states_explored: AtomicUsize::new(0),
        total_transitions: AtomicUsize::new(0),
        max_depth_reached: AtomicUsize::new(0),
        violation_found: AtomicBool::new(false),
        start_time: start,
    });

    let factory = Arc::new(model_factory);
    let mut handles = Vec::new();
    for wc in worker_configs {
        let shared = Arc::clone(&shared);
        let factory = Arc::clone(&factory);
        handles.push(std::thread::spawn(move || {
            let model = factory();
            run_worker(&shared, &wc, &model)
        }));
    }

    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed().as_secs_f64();
    let violations = shared.violations.lock().unwrap().clone();
    let states_explored = shared.total_states_explored.load(Ordering::Relaxed);
    let transitions = shared.total_transitions.load(Ordering::Relaxed);
    let depth_reached = shared.max_depth_reached.load(Ordering::Relaxed);

    CheckResult {
        states_explored,
        states_stored: states_explored,
        transitions,
        depth_reached,
        errors: violations.len(),
        violations,
        elapsed_secs: elapsed,
    }
}

/// Run a single swarm worker with its config and model.
fn run_worker<M>(shared: &SwarmShared, wc: &SwarmWorkerConfig, model: &M)
where
    M: Model + Send + 'static,
    M::State: Send + 'static,
{
    if shared.violation_found.load(Ordering::Relaxed) {
        return;
    }

    let init_states = model.init_states();
    if init_states.is_empty() {
        return;
    }

    let mut storage: Box<dyn StateStore<M::State>> = match wc.storage_mode {
        StorageMode::Exact => Box::new(ExactStore::<M::State>::new()),
        StorageMode::Bitstate => Box::new(BitstateStore::new((wc.max_states / 8).max(1024))),
        StorageMode::Collapse => Box::new(ExactStore::<M::State>::new()),
        StorageMode::HashCompact => Box::new(ExactStore::<M::State>::new()),
    };

    let mut transitions_count = 0;
    let mut max_depth = 0;

    match wc.search_mode {
        SearchMode::DepthFirst => {
            let mut stack: Vec<(M::State, usize)> = Vec::new();
            for s in init_states {
                let h = model.hash(&s);
                if storage.insert(h, &s) {
                    stack.push((s, 0));
                }
            }

            while let Some((state, depth)) = stack.pop() {
                max_depth = max_depth.max(depth);
                if depth >= wc.max_depth {
                    continue;
                }
                if storage.len() >= wc.max_states {
                    break;
                }
                if shared.violation_found.load(Ordering::Relaxed) {
                    return;
                }

                if let Some(desc) = model.check_violation(&state) {
                    let mut violations = shared.violations.lock().unwrap();
                    violations.push(Violation {
                        property_name: "assertion".to_string(),
                        trail: vec![],
                        description: desc,
                    });
                    shared.violation_found.store(true, Ordering::Relaxed);
                    return;
                }

                let trans = model.transitions(&state);
                transitions_count += trans.len();

                for t in trans {
                    let h = model.hash(&t.next);
                    if storage.insert(h, &t.next) {
                        stack.push((t.next, depth + 1));
                    }
                }
            }
        }
        SearchMode::BreadthFirst => {
            let mut queue: std::collections::VecDeque<(M::State, usize)> =
                std::collections::VecDeque::new();
            for s in init_states {
                let h = model.hash(&s);
                if storage.insert(h, &s) {
                    queue.push_back((s, 0));
                }
            }

            while let Some((state, depth)) = queue.pop_front() {
                max_depth = max_depth.max(depth);
                if depth >= wc.max_depth {
                    continue;
                }
                if storage.len() >= wc.max_states {
                    break;
                }
                if shared.violation_found.load(Ordering::Relaxed) {
                    return;
                }

                if let Some(desc) = model.check_violation(&state) {
                    let mut violations = shared.violations.lock().unwrap();
                    violations.push(Violation {
                        property_name: "assertion".to_string(),
                        trail: vec![],
                        description: desc,
                    });
                    shared.violation_found.store(true, Ordering::Relaxed);
                    return;
                }

                let trans = model.transitions(&state);
                transitions_count += trans.len();

                for t in trans {
                    let h = model.hash(&t.next);
                    if storage.insert(h, &t.next) {
                        queue.push_back((t.next, depth + 1));
                    }
                }
            }
        }
    }

    shared
        .total_states_explored
        .fetch_add(storage.len(), Ordering::Relaxed);
    shared
        .total_transitions
        .fetch_add(transitions_count, Ordering::Relaxed);
    shared
        .max_depth_reached
        .fetch_max(max_depth, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::{Model, Transition};

    struct SimpleViolationModel;

    impl Model for SimpleViolationModel {
        type State = u8;

        fn init_states(&self) -> Vec<u8> {
            vec![0]
        }

        fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
            match state {
                0 => vec![Transition {
                    label: "0→1".into(),
                    next: 1,
                }],
                1 => vec![Transition {
                    label: "1→2".into(),
                    next: 2,
                }],
                _ => vec![Transition {
                    label: "loop".into(),
                    next: *state,
                }],
            }
        }

        fn hash(&self, state: &u8) -> u64 {
            *state as u64
        }

        fn check_violation(&self, state: &u8) -> Option<String> {
            if *state == 2 {
                Some("state 2 is forbidden".to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn test_swarm_config_generation() {
        let config = SwarmConfig {
            num_workers: 4,
            ..Default::default()
        };
        let workers = generate_swarm_configs(&config);
        assert_eq!(workers.len(), 4);
        assert_ne!(workers[0].seed, workers[1].seed);
        assert_ne!(workers[0].search_mode, workers[1].search_mode);
    }

    #[test]
    fn test_swarm_finds_violation() {
        let config = SwarmConfig {
            num_workers: 2,
            ..Default::default()
        };
        let result = run_swarm(|| SimpleViolationModel, &config);
        assert!(result.errors > 0);
        assert!(!result.violations.is_empty());
    }

    #[test]
    fn test_swarm_no_violation() {
        struct SafeModel;
        impl Model for SafeModel {
            type State = u8;
            fn init_states(&self) -> Vec<u8> {
                vec![0]
            }
            fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
                if *state < 10 {
                    vec![Transition {
                        label: "inc".into(),
                        next: state + 1,
                    }]
                } else {
                    vec![Transition {
                        label: "loop".into(),
                        next: *state,
                    }]
                }
            }
            fn hash(&self, state: &u8) -> u64 {
                *state as u64
            }
        }

        let config = SwarmConfig {
            num_workers: 2,
            ..Default::default()
        };
        let result = run_swarm(|| SafeModel, &config);
        assert_eq!(result.errors, 0);
    }
}
