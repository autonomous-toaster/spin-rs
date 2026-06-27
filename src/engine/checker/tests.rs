use super::*;

/// A simple test model with 3 states in a chain: A → B → C (self-loop).
struct ChainModel;

impl Model for ChainModel {
    type State = u8;

    fn init_states(&self) -> Vec<u8> {
        vec![0]
    }

    fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
        match state {
            0 => vec![Transition {
                label: "A→B".into(),
                next: 1,
            }],
            1 => vec![Transition {
                label: "B→C".into(),
                next: 2,
            }],
            2 => vec![Transition {
                label: "C→C".into(),
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
fn test_dfs_chain() {
    let model = ChainModel;
    let checker = CheckerBuilder::new().model(model).build();
    let result = checker.check_dfs();
    assert_eq!(result.states_explored, 3);
    assert_eq!(result.transitions, 3);
    assert_eq!(result.errors, 0);
}

#[test]
fn test_bfs_chain() {
    let model = ChainModel;
    let checker = CheckerBuilder::new()
        .model(model)
        .search_mode(SearchMode::BreadthFirst)
        .build();
    let result = checker.check_bfs();
    assert_eq!(result.states_explored, 3);
}

#[test]
fn test_max_depth_limit() {
    let model = ChainModel;
    let checker = CheckerBuilder::new().model(model).max_depth(1).build();
    let result = checker.check_dfs();
    assert_eq!(result.states_explored, 2);
}

#[test]
fn test_max_states_limit() {
    let model = ChainModel;
    let checker = CheckerBuilder::new().model(model).max_states(2).build();
    let result = checker.check_dfs();
    assert_eq!(result.states_explored, 2);
}

struct ViolationModel;

impl Model for ViolationModel {
    type State = i32;

    fn init_states(&self) -> Vec<i32> {
        vec![0]
    }

    fn transitions(&self, state: &i32) -> Vec<Transition<i32>> {
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

    fn hash(&self, state: &i32) -> u64 {
        *state as u64
    }

    fn check_violation(&self, state: &i32) -> Option<String> {
        if *state == 2 {
            Some("state 2 is forbidden".to_string())
        } else {
            None
        }
    }
}

#[test]
fn test_violation_detection() {
    let model = ViolationModel;
    let checker = CheckerBuilder::new().model(model).build();
    let result = checker.check_dfs();
    assert_eq!(result.errors, 1);
    assert_eq!(result.violations[0].description, "state 2 is forbidden");
    assert!(!result.violations[0].trail.is_empty());
}

#[test]
fn test_violation_with_trail() {
    let model = ViolationModel;
    let checker = CheckerBuilder::new().model(model).build();
    let result = checker.check_dfs();
    let trail = &result.violations[0].trail;
    assert!(trail.contains(&"0→1".to_string()) || trail.contains(&"1→2".to_string()));
}
