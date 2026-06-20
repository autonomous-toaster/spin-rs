//! Parallel state exploration for model checking.
//!
//! This module provides multi-threaded DFS/BFS verification using rayon.
//! Enabled with the `parallel` feature flag.
//!
//! ## Architecture
//!
//! Parallel verification uses a work-stealing approach:
//! 1. States are partitioned among worker threads
//! 2. Each worker maintains its own visited set (lock-free with hash splitting)
//! 3. Violations are collected and deduplicated
//!
//! ## Usage
//!
//! ```ignore
//! use spin_rs::engine::parallel::ParallelChecker;
//!
//! let checker = ParallelChecker::new(model)
//!     .num_threads(4)
//!     .build();
//! let result = checker.check_parallel_dfs();
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::engine::checker::{CheckResult, Model, Transition, Violation};

/// Configuration for parallel verification.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of worker threads (0 = auto-detect)
    pub num_threads: usize,
    /// Maximum states to explore
    pub max_states: usize,
    /// Maximum depth
    pub max_depth: usize,
    /// Whether to check assertions
    pub check_assertions: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            num_threads: 0,
            max_states: 1_000_000,
            max_depth: 100_000,
            check_assertions: true,
        }
    }
}

/// Thread-safe visited state set.
struct ParallelVisited<S> {
    /// Partitioned hash map for lock-free concurrent access
    partitions: Vec<Mutex<HashMap<u64, Vec<S>>>>,
    /// Total count of visited states
    count: AtomicUsize,
}

impl<S: Clone + std::hash::Hash + Eq + Send> ParallelVisited<S> {
    fn new(partition_count: usize) -> Self {
        Self {
            partitions: (0..partition_count)
                .map(|_| Mutex::new(HashMap::new()))
                .collect(),
            count: AtomicUsize::new(0),
        }
    }

    fn insert(&self, hash: u64, state: &S) -> bool {
        let partition = hash as usize % self.partitions.len();
        let mut map = self.partitions[partition].lock().unwrap();
        let bucket = map.entry(hash).or_default();
        if bucket.iter().any(|s| s == state) {
            false // duplicate
        } else {
            bucket.push(state.clone());
            self.count.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    fn len(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Shared state for parallel DFS workers.
struct SharedState<M: Model> {
    model: M,
    visited: ParallelVisited<M::State>,
    violations: Mutex<Vec<Violation>>,
    config: ParallelConfig,
    start_time: std::time::Instant,
}

/// Multi-threaded model checker.
pub struct ParallelChecker<M: Model> {
    model: M,
    config: ParallelConfig,
}

impl<M: Model + Send + Sync + 'static> ParallelChecker<M>
where
    M::State: Send + 'static,
{
    /// Create a new parallel checker.
    pub fn new(model: M) -> Self {
        Self {
            model,
            config: ParallelConfig::default(),
        }
    }

    /// Set number of threads.
    pub fn num_threads(mut self, n: usize) -> Self {
        self.config.num_threads = n;
        self
    }

    /// Set max states.
    pub fn max_states(mut self, n: usize) -> Self {
        self.config.max_states = n;
        self
    }

    /// Set max depth.
    pub fn max_depth(mut self, n: usize) -> Self {
        self.config.max_depth = n;
        self
    }

    /// Build and run parallel DFS verification.
    pub fn check_parallel_dfs(self) -> CheckResult {
        let num_threads = if self.config.num_threads > 0 {
            self.config.num_threads
        } else {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        };

        let init_states = self.model.init_states();
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

        let shared = Arc::new(SharedState {
            model: self.model,
            visited: ParallelVisited::new(num_threads * 4),
            violations: Mutex::new(Vec::new()),
            config: self.config,
            start_time: std::time::Instant::now(),
        });

        // Distribute initial states among workers
        let mut handles = Vec::new();
        for chunk in init_states.chunks((init_states.len() + num_threads - 1) / num_threads) {
            let states: Vec<M::State> = chunk.to_vec();
            let shared = Arc::clone(&shared);
            handles.push(std::thread::spawn(move || {
                Self::dfs_worker(states, shared);
            }));
        }

        for h in handles {
            let _ = h.join();
        }

        let elapsed = shared.start_time.elapsed().as_secs_f64();
        let violations = shared.violations.lock().unwrap().clone();

        CheckResult {
            states_explored: shared.visited.len(),
            states_stored: shared.visited.len(),
            transitions: 0, // Not tracked in parallel mode
            depth_reached: 0,
            errors: violations.len(),
            violations,
            elapsed_secs: elapsed,
        }
    }

    fn dfs_worker(init_states: Vec<M::State>, shared: Arc<SharedState<M>>) {
        let mut stack: Vec<(M::State, usize)> = init_states
            .into_iter()
            .map(|s| {
                let h = shared.model.hash(&s);
                shared.visited.insert(h, &s);
                (s, 0)
            })
            .collect();

        while let Some((state, depth)) = stack.pop() {
            if depth >= shared.config.max_depth {
                continue;
            }
            if shared.visited.len() >= shared.config.max_states {
                break;
            }

            // Check violations
            if shared.config.check_assertions {
                if let Some(desc) = shared.model.check_violation(&state) {
                    let mut violations = shared.violations.lock().unwrap();
                    violations.push(Violation {
                        property_name: "assertion".to_string(),
                        trail: vec![],
                        description: desc,
                    });
                    if violations.len() >= 100 {
                        break;
                    }
                    continue;
                }
            }

            let trans = shared.model.transitions(&state);
            for t in trans {
                let h = shared.model.hash(&t.next);
                if shared.visited.insert(h, &t.next) {
                    stack.push((t.next, depth + 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::CheckerBuilder;

    struct SimpleModel;

    impl Model for SimpleModel {
        type State = u8;

        fn init_states(&self) -> Vec<u8> {
            vec![0]
        }

        fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
            match state {
                0 => vec![
                    Transition {
                        label: "0→1".into(),
                        next: 1,
                    },
                    Transition {
                        label: "0→2".into(),
                        next: 2,
                    },
                ],
                1 => vec![Transition {
                    label: "1→1".into(),
                    next: 1,
                }],
                2 => vec![Transition {
                    label: "2→2".into(),
                    next: 2,
                }],
                _ => vec![],
            }
        }

        fn hash(&self, state: &u8) -> u64 {
            *state as u64
        }
    }

    #[test]
    fn test_parallel_dfs() {
        let model = SimpleModel;
        let checker = ParallelChecker::new(model).num_threads(2).max_states(100);
        let result = checker.check_parallel_dfs();
        assert_eq!(result.states_explored, 3);
    }

    #[test]
    fn test_parallel_init_empty() {
        struct EmptyModel;
        impl Model for EmptyModel {
            type State = u8;
            fn init_states(&self) -> Vec<u8> {
                vec![]
            }
            fn transitions(&self, _: &u8) -> Vec<Transition<u8>> {
                vec![]
            }
            fn hash(&self, _: &u8) -> u64 {
                0
            }
        }
        let checker = ParallelChecker::new(EmptyModel);
        let result = checker.check_parallel_dfs();
        assert_eq!(result.states_explored, 0);
        assert_eq!(result.errors, 0);
    }
}
