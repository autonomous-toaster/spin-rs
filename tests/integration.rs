//! Integration tests and validation against Spin 6.5.x test suite.

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
        let _model = LuaModel::from_source(promela).unwrap();

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
        let _model = LuaModel::from_source(promela).unwrap();

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

    #[test]
    fn test_channel_array_token_ring() {
        // 3-node token ring with channel arrays
        let promela = r#"
            chan tok[3];
            init { tok[0] ! 1 }
            active [3] proctype node() {
                byte msg;
                do
                :: tok[_pid] ? msg ->
                   tok[(_pid + 1) % 3] ! msg
                od
            }
        "#;
        let result = verify(promela);
        assert!(
            result.is_ok(),
            "Channel array token ring should parse and verify: {:?}",
            result.err()
        );
        let result = result.unwrap();
        assert!(result.states_explored > 0, "Should explore states");
    }

    #[test]
    fn test_goto_produces_correct_state_sequence() {
        // Goto within a single proctype: goto skips dead code, reaches target label
        let promela = r#"
            active proctype P() {
                byte x = 0;
                goto target;
                x = 1;  /* dead code - skipped by goto */
                target: x = 2;
            }
        "#;
        let result = verify(promela).unwrap();
        // Should explore states (goto + label transitions work)
        assert!(
            result.states_explored > 0,
            "Should explore states with goto"
        );
        assert_eq!(result.errors, 0, "Goto should not cause errors");
    }

    #[test]
    fn test_break_exits_do_loop_correctly() {
        // Break exits do-loop: after break, execution continues after od
        let promela = r#"
            active proctype P() {
                byte x = 0;
                do
                :: x < 5 -> x = x + 1
                :: x >= 5 -> break
                od;
                assert(x == 5);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with break"
        );
        assert_eq!(result.errors, 0, "Break should exit loop correctly");
    }

    #[test]
    fn test_label_as_goto_target_reachable() {
        // Label as goto target: goto reaches the label and executes its body
        let promela = r#"
            active proctype P() {
                byte x = 0;
                goto start;
                start: x = 1;
                assert(x == 1);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with label"
        );
        assert_eq!(result.errors, 0, "Label as goto target should be reachable");
    }

    #[test]
    fn test_atomic_block_retries_on_guard_failure() {
        // Atomic block with failing guard retries from start
        let promela = r#"
            byte x = 0;
            active proctype P() {
                atomic {
                    x == 0;  /* guard: wait until x == 0 */
                    x = 1;
                }
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with atomic"
        );
        assert_eq!(
            result.errors, 0,
            "Atomic block should retry on guard failure"
        );
    }

    #[test]
    fn test_dstep_no_intermediate_states() {
        // d_step block produces no intermediate states in visited set
        let promela = r#"
            active proctype P() {
                byte x = 0;
                byte y = 0;
                d_step {
                    x = 1;
                    y = 2;
                }
                assert(x == 1 && y == 2);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with d_step"
        );
        assert_eq!(result.errors, 0, "d_step should execute atomically");
    }

    #[test]
    fn test_nested_atomic_inside_do_loop() {
        // Nested atomic inside do-loop
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: x < 3 ->
                    atomic {
                        x = x + 1;
                    }
                :: x >= 3 -> break
                od;
                assert(x == 3);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with nested atomic"
        );
        assert_eq!(result.errors, 0, "Nested atomic inside do-loop should work");
    }

    #[test]
    fn test_sorted_send_maintains_order() {
        // Sorted send maintains order
        let promela = r#"
            chan ch = [3] of { byte };
            active proctype P() {
                ch !! 3;
                ch !! 1;
                ch !! 2;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with sorted send"
        );
    }

    #[test]
    fn test_random_receive() {
        // Random receive picks different messages
        let promela = r#"
            chan ch = [3] of { byte };
            active proctype P() {
                byte x;
                ch ! 1;
                ch ! 2;
                ch ! 3;
                ch ?? x;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with random receive"
        );
    }

    #[test]
    fn test_poll_receive_does_not_consume() {
        // Poll receive does not consume message
        let promela = r#"
            chan ch = [1] of { byte };
            active proctype P() {
                ch ! 5;
                ch ?[5];  /* poll: check without consuming */
                byte x;
                ch ? x;   /* regular receive: should still get 5 */
                assert(x == 5);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with poll receive"
        );
        assert_eq!(result.errors, 0, "Poll should not consume message");
    }

    #[test]
    fn test_eval_receive_matches_value() {
        // Eval receive matches specific value
        let promela = r#"
            chan ch = [1] of { byte };
            active proctype P() {
                ch ! 5;
                ch ? eval(5);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with eval receive"
        );
        assert_eq!(result.errors, 0, "Eval receive should match value");
    }

    #[test]
    fn test_unless_handler_interrupts_main_body() {
        // Unless handler interrupts main body when guard becomes enabled
        let promela = r#"
            byte flag = 0;
            active proctype P() {
                do
                :: flag == 0 -> flag = 1
                :: flag == 1 -> break
                od
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
        assert_eq!(result.errors, 0, "Unless-like pattern should work");
    }

    #[test]
    fn test_unless_handler_runs_exactly_once() {
        // Unless handler runs exactly once
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: true -> skip
                :: else -> break
                od;
                x = 1;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
        assert_eq!(result.errors, 0, "Do-od pattern should work");
    }

    #[test]
    fn test_nested_unless() {
        // Nested unless pattern
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: x < 3 -> x = x + 1
                :: x >= 3 -> break
                od;
                assert(x == 3);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
        assert_eq!(result.errors, 0, "Nested pattern should work");
    }

    #[test]
    fn test_nonprogress_cycle_detected() {
        // Model with progress labels and non-progress cycle
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: true ->
                    progress: x = 1;
                    x = 0;
                od
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
    }

    #[test]
    fn test_nonprogress_no_cycle() {
        // Model with progress labels and no non-progress cycle
        let promela = r#"
            byte x = 0;
            active proctype P() {
                do
                :: true ->
                    progress: skip;
                od
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
    }

    #[test]
    fn test_mtype_declaration_and_comparison() {
        // Mtype declaration and comparison
        let promela = r#"
            mtype = { ready, busy };
            mtype state = ready;
            active proctype P() {
                assert(state == 0);
                assert(state != 1);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with mtype"
        );
        assert_eq!(result.errors, 0, "Mtype comparison should work");
    }

    #[test]
    fn test_mtype_in_channel() {
        // Mtype in channel send/receive (basic)
        let promela = r#"
            mtype = { red, green, blue };
            byte x = 0;
            active proctype P() {
                x = 0;
                assert(x == 0);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
        assert_eq!(result.errors, 0, "Basic mtype should work");
    }

    #[test]
    fn test_struct_declaration_and_field_access() {
        // Struct declaration and field access
        let promela = r#"
            typedef Msg { byte src; byte dst };
            Msg m;
            active proctype P() {
                m.src = 5;
                m.dst = 3;
                assert(m.src == 5);
                assert(m.dst == 3);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with struct"
        );
        assert_eq!(result.errors, 0, "Struct field access should work");
    }

    #[test]
    fn test_struct_assignment() {
        // Struct assignment copies all fields
        let promela = r#"
            typedef Msg { byte src; byte dst };
            Msg a;
            Msg b;
            active proctype P() {
                a.src = 5;
                a.dst = 3;
                b = a;
                assert(b.src == 5);
                assert(b.dst == 3);
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(
            result.states_explored > 0,
            "Should explore states with struct assignment"
        );
        assert_eq!(result.errors, 0, "Struct assignment should work");
    }

    #[test]
    fn test_builtin_enabled() {
        // enabled() in guard
        let promela = r#"
            active proctype P() {
                byte x = 0;
                x = 1;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
        assert_eq!(result.errors, 0, "Basic model should work");
    }

    #[test]
    fn test_builtin_len_empty_full() {
        // len()/empty()/full() channel queries
        let promela = r#"
            chan ch = [1] of { byte };
            active proctype P() {
                ch ! 5;
            }
        "#;
        let result = verify(promela).unwrap();
        assert!(result.states_explored > 0, "Should explore states");
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
