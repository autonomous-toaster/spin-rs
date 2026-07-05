//! Interactive simulation: step-by-step model execution with user choice.
//!
//! Provides an `InteractiveSimulator` that wraps a model and lets the user
//! step through transitions interactively, with step-back (undo) and state
//! inspection capabilities.

use crate::engine::checker::{Model, Transition};

/// A recorded step in the simulation history.
#[derive(Debug, Clone)]
struct HistoryStep<S> {
    state: S,
    transition_label: String,
}

/// Interactive simulator for step-by-step model execution.
///
/// Wraps a model and provides:
/// - `step()`: display current state, list enabled transitions, read user choice
/// - `step_back()`: undo the last step
/// - `inspect()`: display variable values at current step
#[derive(Debug)]
pub struct InteractiveSimulator<M: Model> {
    model: M,
    current_state: Option<M::State>,
    history: Vec<HistoryStep<M::State>>,
}

impl<M: Model> InteractiveSimulator<M> {
    /// Create a new interactive simulator from a model.
    pub fn new(model: M) -> Self {
        let init_states = model.init_states();
        let current_state = init_states.into_iter().next();
        Self {
            model,
            current_state,
            history: Vec::new(),
        }
    }

    /// Get a reference to the current state, if any.
    pub fn current_state(&self) -> Option<&M::State> {
        self.current_state.as_ref()
    }

    /// Get the number of steps taken so far.
    pub fn step_count(&self) -> usize {
        self.history.len()
    }

    /// Get the enabled transitions from the current state.
    pub fn enabled_transitions(&self) -> Vec<Transition<M::State>> {
        match &self.current_state {
            Some(state) => self.model.transitions(state),
            None => vec![],
        }
    }

    /// Execute a single step by choosing a transition by index.
    /// Returns the label of the transition taken, or None if no transition.
    pub fn step(&mut self, choice_index: usize) -> Option<String> {
        let state = self.current_state.as_ref()?;
        let transitions = self.model.transitions(state);

        if choice_index >= transitions.len() {
            return None;
        }

        let transition = transitions[choice_index].clone();
        let prev_state = self.current_state.take().unwrap();

        self.history.push(HistoryStep {
            state: prev_state,
            transition_label: transition.label.clone(),
        });

        self.current_state = Some(transition.next);
        Some(transition.label)
    }

    /// Step back to the previous state (undo last step).
    /// Returns true if a step was undone, false if already at the initial state.
    pub fn step_back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.current_state = Some(prev.state);
            true
        } else {
            false
        }
    }

    /// Display the current state information.
    /// Uses the model's `state_to_string` if available, otherwise shows the hash.
    pub fn display_state(&self) {
        match &self.current_state {
            Some(state) => {
                if let Some(state_str) = self.model.state_to_string(state) {
                    println!("Current state: {}", state_str);
                } else {
                    let hash = self.model.hash(state);
                    println!("Current state (hash: {:016x})", hash);
                }
            }
            None => {
                println!("No current state (model may have no initial states)");
            }
        }
    }

    /// Display all enabled transitions with their indices.
    pub fn display_transitions(&self) {
        let transitions = self.enabled_transitions();
        if transitions.is_empty() {
            println!("No enabled transitions (deadlock)");
            return;
        }

        println!("\nEnabled transitions:");
        for (i, t) in transitions.iter().enumerate() {
            let next_hash = self.model.hash(&t.next);
            println!("  {:3}: {} (next hash: {:016x})", i, t.label, next_hash);
        }
    }

    /// Display the simulation history.
    pub fn display_history(&self) {
        if self.history.is_empty() {
            println!("No steps taken yet.");
            return;
        }

        println!("\nSimulation history:");
        for (i, step) in self.history.iter().enumerate() {
            let hash = self.model.hash(&step.state);
            println!(
                "  {:3}: {} (state hash: {:016x})",
                i + 1,
                step.transition_label,
                hash
            );
        }
    }

    /// Inspect the current state: display all variable values.
    /// Uses the model's `state_to_string` for detailed inspection.
    pub fn inspect(&self) {
        match &self.current_state {
            Some(state) => {
                println!("\n=== State Inspection ===");
                println!("Step: {}", self.history.len());

                if let Some(state_str) = self.model.state_to_string(state) {
                    println!("Variables: {}", state_str);
                } else {
                    let hash = self.model.hash(state);
                    println!("State hash: {:016x}", hash);
                    println!("(State-to-string not available for this model)");
                }

                println!("History depth: {}", self.history.len());
            }
            None => {
                println!("No state to inspect");
            }
        }
    }

    /// Run the interactive simulation loop.
    /// Returns when the user chooses to quit or the simulation ends.
    pub fn run_interactive(&mut self) {
        println!("\n=== Interactive Simulation ===");
        println!("Commands:");
        println!("  <N>       - take transition N (0-indexed)");
        println!("  b         - step back (undo)");
        println!("  i         - inspect current state");
        println!("  h         - show history");
        println!("  t         - list enabled transitions");
        println!("  s         - show current state");
        println!("  q         - quit");
        println!();

        loop {
            self.display_state();
            self.display_transitions();

            print!("\n> ");
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let input = input.trim();

            match input {
                "q" | "quit" => {
                    println!("Exiting interactive simulation.");
                    break;
                }
                "b" | "back" | "undo" => {
                    if self.step_back() {
                        println!("Stepped back.");
                    } else {
                        println!("Already at initial state, cannot step back.");
                    }
                }
                "i" | "inspect" => {
                    self.inspect();
                }
                "h" | "history" => {
                    self.display_history();
                }
                "t" | "transitions" => {
                    self.display_transitions();
                }
                "s" | "state" => {
                    self.display_state();
                }
                _ => {
                    // Try to parse as a transition index
                    if let Ok(index) = input.parse::<usize>() {
                        let transitions = self.enabled_transitions();
                        if index < transitions.len() {
                            let label = self.step(index).unwrap_or_default();
                            println!("Took transition {}: {}", index, label);
                        } else {
                            println!(
                                "Invalid transition index {}. Valid range: 0-{}",
                                index,
                                transitions.len().saturating_sub(1)
                            );
                        }
                    } else if !input.is_empty() {
                        println!("Unknown command: '{}'", input);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::checker::{Model, Transition};

    /// A simple test model with branching transitions.
    struct BranchModel;

    impl Model for BranchModel {
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

        fn state_to_string(&self, state: &u8) -> Option<String> {
            Some(format!("x={}", state))
        }
    }

    #[test]
    fn test_interactive_simulator_creation() {
        let model = BranchModel;
        let sim = InteractiveSimulator::new(model);
        assert_eq!(sim.step_count(), 0);
        assert!(sim.current_state().is_some());
        assert_eq!(*sim.current_state().unwrap(), 0);
    }

    #[test]
    fn test_interactive_step() {
        let model = BranchModel;
        let mut sim = InteractiveSimulator::new(model);

        // Take transition 0: 0→1
        let label = sim.step(0);
        assert_eq!(label, Some("0→1".into()));
        assert_eq!(*sim.current_state().unwrap(), 1);
        assert_eq!(sim.step_count(), 1);

        // Take transition 0 again: 1→1
        let label = sim.step(0);
        assert_eq!(label, Some("1→1".into()));
        assert_eq!(*sim.current_state().unwrap(), 1);
        assert_eq!(sim.step_count(), 2);
    }

    #[test]
    fn test_interactive_step_back() {
        let model = BranchModel;
        let mut sim = InteractiveSimulator::new(model);

        sim.step(0); // 0→1
        assert_eq!(*sim.current_state().unwrap(), 1);

        assert!(sim.step_back());
        assert_eq!(*sim.current_state().unwrap(), 0);
        assert_eq!(sim.step_count(), 0);

        // Cannot step back from initial state
        assert!(!sim.step_back());
    }

    #[test]
    fn test_interactive_inspect() {
        let model = BranchModel;
        let sim = InteractiveSimulator::new(model);

        // inspect should work without panicking
        sim.inspect();
    }

    #[test]
    fn test_interactive_enabled_transitions() {
        let model = BranchModel;
        let sim = InteractiveSimulator::new(model);

        let transitions = sim.enabled_transitions();
        assert_eq!(transitions.len(), 2);
        assert_eq!(transitions[0].label, "0→1");
        assert_eq!(transitions[1].label, "0→2");
    }

    #[test]
    fn test_interactive_invalid_choice() {
        let model = BranchModel;
        let mut sim = InteractiveSimulator::new(model);

        // Out of bounds index
        assert!(sim.step(99).is_none());
        assert_eq!(sim.step_count(), 0);
    }

    #[test]
    fn test_interactive_display_methods() {
        let model = BranchModel;
        let sim = InteractiveSimulator::new(model);

        // These should not panic
        sim.display_state();
        sim.display_transitions();
        sim.display_history();
    }

    #[test]
    fn test_interactive_follows_chosen_path() {
        let model = BranchModel;
        let mut sim = InteractiveSimulator::new(model);

        // User chooses transition 1 (0→2) instead of transition 0 (0→1)
        let label = sim.step(1);
        assert_eq!(label, Some("0→2".into()));
        assert_eq!(*sim.current_state().unwrap(), 2);
        assert_eq!(sim.step_count(), 1);

        // Verify history shows the chosen path
        sim.display_history();

        // Step back and choose the other path
        assert!(sim.step_back());
        assert_eq!(*sim.current_state().unwrap(), 0);

        // Now choose transition 0 (0→1)
        let label = sim.step(0);
        assert_eq!(label, Some("0→1".into()));
        assert_eq!(*sim.current_state().unwrap(), 1);
        assert_eq!(sim.step_count(), 1);
    }

    #[test]
    fn test_interactive_multi_step_path() {
        let model = BranchModel;
        let mut sim = InteractiveSimulator::new(model);

        // Follow a multi-step path: 0→1, then 1→1, then back to 0, then 0→2
        sim.step(0); // 0→1
        assert_eq!(*sim.current_state().unwrap(), 1);

        sim.step(0); // 1→1
        assert_eq!(*sim.current_state().unwrap(), 1);

        sim.step_back(); // back to 1 (from 1→1)
        assert_eq!(*sim.current_state().unwrap(), 1);

        sim.step_back(); // back to 0
        assert_eq!(*sim.current_state().unwrap(), 0);

        sim.step(1); // 0→2
        assert_eq!(*sim.current_state().unwrap(), 2);

        assert_eq!(sim.step_count(), 1); // only one forward step after undo
    }
}
