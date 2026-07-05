//! Parallel BFS state exploration using crossbeam channels.
//!
//! ## Architecture
//!
//! - BFS frontier is a concurrent queue (crossbeam channel)
//! - Visited set uses a Mutex-guarded HashMap (avoids Sync requirement on state type)
//! - Workers pop states from the frontier, expand transitions, and push new states
//! - Each worker gets its own model via a factory closure

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam::channel::{bounded, Receiver, Sender};

use crate::engine::checker::{CheckResult, Model, Violation};

/// Configuration for parallel BFS.
#[derive(Debug, Clone)]
pub struct ParallelBfsConfig {
    pub num_threads: usize,
    pub max_states: usize,
    pub max_depth: usize,
    pub check_assertions: bool,
    pub channel_capacity: usize,
}

impl Default for ParallelBfsConfig {
    fn default() -> Self {
        Self {
            num_threads: 0,
            max_states: 1_000_000,
            max_depth: 100_000,
            check_assertions: true,
            channel_capacity: 65536,
        }
    }
}

/// Shared state for parallel BFS workers.
struct BfsShared<S> {
    visited: Mutex<std::collections::HashMap<u64, Vec<S>>>,
    visited_count: AtomicUsize,
    violations: Mutex<Vec<Violation>>,
    violation_found: AtomicBool,
    config: ParallelBfsConfig,
    start_time: Instant,
}

/// Run parallel BFS verification using a model factory.
pub fn run_parallel_bfs<M, F>(model_factory: F, config: &ParallelBfsConfig) -> CheckResult
where
    M: Model + 'static,
    M::State: Send + 'static,
    F: Fn() -> M + Send + Sync + 'static,
{
    let start = Instant::now();
    let num_threads = if config.num_threads > 0 {
        config.num_threads
    } else {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    };

    let factory = Arc::new(model_factory);

    // Create initial model to get init states
    let model = factory();
    let init_states = model.init_states();
    if init_states.is_empty() {
        return CheckResult {
            states_explored: 0,
            states_stored: 0,
            transitions: 0,
            depth_reached: 0,
            errors: 0,
            violations: vec![],
            elapsed_secs: 0.0,
        };
    }

    let shared = Arc::new(BfsShared {
        visited: Mutex::new(std::collections::HashMap::new()),
        visited_count: AtomicUsize::new(0),
        violations: Mutex::new(Vec::new()),
        violation_found: AtomicBool::new(false),
        config: config.clone(),
        start_time: start,
    });

    // Create the BFS frontier channel
    let (tx, rx): (Sender<(M::State, usize)>, Receiver<(M::State, usize)>) =
        bounded(config.channel_capacity);

    // Seed initial states
    {
        let mut visited = shared.visited.lock().unwrap();
        for s in init_states {
            let h = model.hash(&s);
            let bucket = visited.entry(h).or_default();
            if !bucket.iter().any(|x| x == &s) {
                bucket.push(s.clone());
                shared.visited_count.fetch_add(1, Ordering::Relaxed);
                let _ = tx.send((s, 0));
            }
        }
    }
    drop(model);

    // Spawn worker threads
    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let shared = Arc::clone(&shared);
        let rx = rx.clone();
        let tx = tx.clone();
        let factory = Arc::clone(&factory);
        handles.push(std::thread::spawn(move || {
            let model = factory();
            bfs_worker(&shared, &rx, &tx, &model);
        }));
    }

    drop(tx);

    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed().as_secs_f64();
    let violations = shared.violations.lock().unwrap().clone();
    let states_explored = shared.visited_count.load(Ordering::Relaxed);

    CheckResult {
        states_explored,
        states_stored: states_explored,
        transitions: 0,
        depth_reached: 0,
        errors: violations.len(),
        violations,
        elapsed_secs: elapsed,
    }
}

/// BFS worker: pop states from the frontier, expand, push new states.
fn bfs_worker<M>(
    shared: &BfsShared<M::State>,
    rx: &Receiver<(M::State, usize)>,
    tx: &Sender<(M::State, usize)>,
    model: &M,
) where
    M: Model + 'static,
    M::State: Send + 'static,
{
    loop {
        if shared.violation_found.load(Ordering::Relaxed) {
            break;
        }

        let (state, depth) = match rx.recv() {
            Ok(item) => item,
            Err(_) => break,
        };

        if depth >= shared.config.max_depth {
            continue;
        }
        if shared.visited_count.load(Ordering::Relaxed) >= shared.config.max_states {
            break;
        }

        // Check for violations
        if shared.config.check_assertions
            && let Some(desc) = model.check_violation(&state) {
                let mut violations = shared.violations.lock().unwrap();
                violations.push(Violation {
                    property_name: "assertion".to_string(),
                    trail: vec![],
                    description: desc,
                });
                shared.violation_found.store(true, Ordering::Relaxed);
                break;
            }

        // Expand transitions
        let transitions = model.transitions(&state);
        for t in transitions {
            let h = model.hash(&t.next);
            let mut visited = shared.visited.lock().unwrap();
            let bucket = visited.entry(h).or_default();
            if !bucket.iter().any(|x| x == &t.next) {
                bucket.push(t.next.clone());
                shared.visited_count.fetch_add(1, Ordering::Relaxed);
                drop(visited);
                let _ = tx.send((t.next, depth + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::{CheckerBuilder, Model, SearchMode, Transition};

    struct SimpleModel;

    impl Model for SimpleModel {
        type State = u8;

        fn init_states(&self) -> Vec<u8> {
            vec![0]
        }

        fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
            match state {
                0 => vec![
                    Transition { label: "0→1".into(), next: 1 },
                    Transition { label: "0→2".into(), next: 2 },
                ],
                1 => vec![Transition { label: "1→1".into(), next: 1 }],
                2 => vec![Transition { label: "2→2".into(), next: 2 }],
                _ => vec![],
            }
        }

        fn hash(&self, state: &u8) -> u64 {
            *state as u64
        }
    }

    #[test]
    fn test_parallel_bfs_simple() {
        let config = ParallelBfsConfig {
            num_threads: 2,
            max_states: 100,
            ..Default::default()
        };
        let result = run_parallel_bfs(|| SimpleModel, &config);
        assert_eq!(result.states_explored, 3);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_parallel_bfs_matches_sequential() {
        let config = ParallelBfsConfig {
            num_threads: 2,
            max_states: 100,
            ..Default::default()
        };
        let parallel_result = run_parallel_bfs(|| SimpleModel, &config);

        let seq_model = SimpleModel;
        let checker = CheckerBuilder::new()
            .model(seq_model)
            .search_mode(SearchMode::BreadthFirst)
            .build();
        let seq_result = checker.check_bfs();

        assert_eq!(parallel_result.states_explored, seq_result.states_explored);
        assert_eq!(parallel_result.errors, seq_result.errors);
    }

    #[test]
    fn test_parallel_bfs_empty_init() {
        struct EmptyModel;
        impl Model for EmptyModel {
            type State = u8;
            fn init_states(&self) -> Vec<u8> { vec![] }
            fn transitions(&self, _: &u8) -> Vec<Transition<u8>> { vec![] }
            fn hash(&self, _: &u8) -> u64 { 0 }
        }

        let config = ParallelBfsConfig::default();
        let result = run_parallel_bfs(|| EmptyModel, &config);
        assert_eq!(result.states_explored, 0);
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_parallel_bfs_violation() {
        struct ViolationModel;
        impl Model for ViolationModel {
            type State = i32;
            fn init_states(&self) -> Vec<i32> { vec![0] }
            fn transitions(&self, state: &i32) -> Vec<Transition<i32>> {
                match state {
                    0 => vec![Transition { label: "0→1".into(), next: 1 }],
                    1 => vec![Transition { label: "1→2".into(), next: 2 }],
                    _ => vec![Transition { label: "loop".into(), next: *state }],
                }
            }
            fn hash(&self, state: &i32) -> u64 { *state as u64 }
            fn check_violation(&self, state: &i32) -> Option<String> {
                if *state == 2 { Some("forbidden".into()) } else { None }
            }
        }

        let config = ParallelBfsConfig {
            num_threads: 2,
            ..Default::default()
        };
        let result = run_parallel_bfs(|| ViolationModel, &config);
        assert_eq!(result.errors, 1);
    }
}
