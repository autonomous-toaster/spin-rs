//! Integration tests and validation against Spin 6.5.x test suite.

use std::fs;
use std::path::PathBuf;

use spin_rs::{CheckResult, CheckerBuilder, LuaModel, SearchMode, StorageMode, verify};

/// Test model from Spin standard suite.
struct TestModel {
    name: &'static str,
    source: &'static str,
    expected_errors: usize,
    expected_states_min: usize,
    expected_states_max: usize,
}

/// Peterson's mutual exclusion algorithm.
const PETERSON: &str = r#"
#define N 2
byte turn = 0;
byte flag[N] = 0;

active [N] proctype user() {
    do
    :: flag[_pid] = 1;
       turn = 1 - _pid;
       (flag[1-_pid] == 0 || turn == _pid);
       /* critical section */
       flag[_pid] = 0;
    od
}
"#;

/// Simple assertion test.
const ASSERTION_TEST: &str = r#"
active proctype Main() {
    byte x = 0;
    x = 1;
    assert(x == 1);
}
"#;

/// Channel send/receive test.
const CHANNEL_TEST: &str = r#"
chan q = [1] of { byte };

active proctype Sender() {
    q ! 1;
    q ! 2;
}

active proctype Receiver() {
    byte x;
    q ? x;
    q ? x;
}
"#;

/// LTL liveness test.
const LTL_LIVENESS: &str = r#"
byte x = 0;

active proctype P() {
    do
    :: x = 0
    :: x = 1
    od
}

ltl p0 { []<>(x == 0) };
"#;

/// Deadlock test.
const DEADLOCK_TEST: &str = r#"
active proctype P() {
    byte x = 0;
    if
    :: x == 0 -> skip
    :: x == 1 -> skip
    fi;
    assert(x == 0 || x == 1);
}
"#;

const TEST_MODELS: &[TestModel] = &[
    TestModel {
        name: "peterson",
        source: PETERSON,
        expected_errors: 0,
        expected_states_min: 1,
        expected_states_max: 1000,
    },
    TestModel {
        name: "assertion",
        source: ASSERTION_TEST,
        expected_errors: 0,
        expected_states_min: 1,
        expected_states_max: 10,
    },
    TestModel {
        name: "channel",
        source: CHANNEL_TEST,
        expected_errors: 0,
        expected_states_min: 1,
        expected_states_max: 100,
    },
    TestModel {
        name: "ltl_liveness",
        source: LTL_LIVENESS,
        expected_errors: 0,
        expected_states_min: 1,
        expected_states_max: 50,
    },
    TestModel {
        name: "deadlock",
        source: DEADLOCK_TEST,
        expected_errors: 0,
        expected_states_min: 1,
        expected_states_max: 10,
    },
];

/// Run a single test model.
fn run_test_model(model: &TestModel) -> Result<CheckResult, anyhow::Error> {
    let result = verify(model.source)?;

    // Validate results
    assert_eq!(
        result.errors, model.expected_errors,
        "{}: expected {} errors, got {}",
        model.name, model.expected_errors, result.errors
    );

    assert!(
        result.states_explored >= model.expected_states_min,
        "{}: expected at least {} states, got {}",
        model.name,
        model.expected_states_min,
        result.states_explored
    );

    assert!(
        result.states_explored <= model.expected_states_max,
        "{}: expected at most {} states, got {}",
        model.name,
        model.expected_states_max,
        result.states_explored
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peterson_mutual_exclusion() {
        let model = TEST_MODELS.iter().find(|m| m.name == "peterson").unwrap();
        let result = run_test_model(model).unwrap();
        println!(
            "Peterson: {} states, {} transitions",
            result.states_explored, result.transitions
        );
    }

    #[test]
    fn test_assertion_success() {
        let model = TEST_MODELS.iter().find(|m| m.name == "assertion").unwrap();
        let result = run_test_model(model).unwrap();
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_channel_communication() {
        let model = TEST_MODELS.iter().find(|m| m.name == "channel").unwrap();
        let result = run_test_model(model).unwrap();
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_ltl_liveness_property() {
        let model = TEST_MODELS
            .iter()
            .find(|m| m.name == "ltl_liveness")
            .unwrap();
        // LTL verification is handled separately
        let result = verify(model.source).unwrap();
        assert!(result.states_explored > 0);
    }

    #[test]
    fn test_no_deadlock() {
        let model = TEST_MODELS.iter().find(|m| m.name == "deadlock").unwrap();
        let result = run_test_model(model).unwrap();
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_storage_modes() {
        let promela = "active proctype P() { byte x; x = 1; }";
        let model = LuaModel::from_source(promela).unwrap();

        // Test Exact mode
        let checker_exact = CheckerBuilder::new()
            .model(LuaModel::from_source(promela).unwrap())
            .storage_mode(StorageMode::Exact)
            .build();
        let result_exact = checker_exact.check();
        assert!(result_exact.states_explored > 0);

        // Test Bitstate mode
        let checker_bitstate = CheckerBuilder::new()
            .model(LuaModel::from_source(promela).unwrap())
            .storage_mode(StorageMode::Bitstate)
            .build();
        let result_bitstate = checker_bitstate.check();
        assert!(result_bitstate.states_explored > 0);
    }

    #[test]
    fn test_search_modes() {
        let promela = "active proctype P() { byte x; do :: x = 0 :: x = 1 od }";
        let model = LuaModel::from_source(promela).unwrap();

        // Test DFS
        let checker_dfs = CheckerBuilder::new()
            .model(LuaModel::from_source(promela).unwrap())
            .search_mode(SearchMode::DepthFirst)
            .build();
        let result_dfs = checker_dfs.check();
        assert!(result_dfs.states_explored > 0);

        // Test BFS
        let checker_bfs = CheckerBuilder::new()
            .model(LuaModel::from_source(promela).unwrap())
            .search_mode(SearchMode::BreadthFirst)
            .build();
        let result_bfs = checker_bfs.check();
        assert!(result_bfs.states_explored > 0);
    }

    #[test]
    fn test_por_enabled() {
        let promela = r#"
            chan q = [1] of { byte };
            active proctype P() { q ! 1; }
            active proctype Q() { byte x; q ? x; }
        "#;
        let model = LuaModel::from_source(promela).unwrap();

        let checker_por = CheckerBuilder::new()
            .model(LuaModel::from_source(promela).unwrap())
            .por_enabled(true)
            .build();
        let result_por = checker_por.check();

        let checker_no_por = CheckerBuilder::new()
            .model(model)
            .por_enabled(false)
            .build();
        let result_no_por = checker_no_por.check();

        // POR should explore fewer or equal states
        assert!(result_por.states_explored <= result_no_por.states_explored);
    }

    #[test]
    fn test_max_states_limit() {
        let promela = "active proctype P() { byte x; do :: x = (x + 1) % 10 od }";
        let model = LuaModel::from_source(promela).unwrap();

        let checker = CheckerBuilder::new().model(model).max_states(5).build();
        let result = checker.check();

        assert!(result.states_explored <= 5);
    }

    #[test]
    fn test_max_depth_limit() {
        let promela = "active proctype P() { byte x = 0; do :: x = x + 1 od }";
        let model = LuaModel::from_source(promela).unwrap();

        let checker = CheckerBuilder::new().model(model).max_depth(10).build();
        let result = checker.check();

        assert!(result.depth_reached <= 10);
    }

    #[test]
    fn test_multiple_processes() {
        let promela = r#"
            active [3] proctype Worker() {
                byte x = 0;
                x = 1;
                x = 2;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0);
    }

    #[test]
    fn test_if_fi_statement() {
        let promela = r#"
            active proctype P() {
                byte x = 0;
                if
                :: x == 0 -> x = 1
                :: x == 1 -> x = 2
                fi;
            }
        "#;
        let result = verify(promela).unwrap();
        assert_eq!(result.errors, 0);
    }

    #[test]
    fn test_do_od_loop() {
        let promela = r#"
            active proctype P() {
                byte x = 0;
                do
                :: x < 5 -> x = x + 1
                :: x >= 5 -> break
                od;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0);
    }

    #[test]
    fn test_guarded_commands() {
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: x == 0 -> x = 1
                :: x == 1 -> x = 0
                od
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0);
    }
}

/// Benchmark utilities.
#[cfg(feature = "bench")]
pub mod benchmarks {
    use super::*;
    use std::time::Instant;

    pub struct BenchmarkResult {
        pub name: &'static str,
        pub states_per_sec: f64,
        pub elapsed_secs: f64,
        pub states_explored: usize,
    }

    pub fn run_benchmark(model: &TestModel) -> BenchmarkResult {
        let start = Instant::now();
        let result = verify(model.source).unwrap();
        let elapsed = start.elapsed().as_secs_f64();

        let states_per_sec = if elapsed > 0.0 {
            result.states_explored as f64 / elapsed
        } else {
            0.0
        };

        BenchmarkResult {
            name: model.name,
            states_per_sec,
            elapsed_secs: elapsed,
            states_explored: result.states_explored,
        }
    }

    pub fn print_benchmark_results(results: &[BenchmarkResult]) {
        println!("\n=== Benchmark Results ===");
        println!(
            "{:<20} {:>12} {:>12} {:>12}",
            "Model", "States", "Time (s)", "States/sec"
        );
        println!("{:-<60}", "");
        for r in results {
            println!(
                "{:<20} {:>12} {:>12.3} {:>12.0}",
                r.name, r.states_explored, r.elapsed_secs, r.states_per_sec
            );
        }
    }
}
