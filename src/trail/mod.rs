//! Trail I/O: error trail generation, serialization, and replay.
//!
//! Implements Spin-compatible trail format for counterexample visualization.
//! Trails record the sequence of transitions taken to reach a violation,
//! enabling step-by-step replay with state inspection.

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::checker::Violation;

/// A single step in an error trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailStep {
    /// Transition label (e.g., "P:1" for proctype P, line 1).
    pub label: String,
    /// State hash at this point.
    pub state_hash: u64,
    /// Optional state snapshot (for detailed replay).
    pub state_snapshot: Option<String>,
    /// Source location (file:line) if available.
    pub source_loc: Option<String>,
}

/// Complete error trail with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTrail {
    /// Property name that was violated.
    pub property_name: String,
    /// Description of the violation.
    pub description: String,
    /// Sequence of steps from initial state to violation.
    pub steps: Vec<TrailStep>,
    /// Total states explored during verification.
    pub states_explored: usize,
    /// Maximum depth reached.
    pub depth_reached: usize,
}

impl ErrorTrail {
    /// Create a new error trail from a violation and step sequence.
    pub fn new(
        violation: Violation,
        state_hashes: Vec<u64>,
        states_explored: usize,
        depth_reached: usize,
    ) -> Self {
        let steps = violation
            .trail
            .iter()
            .zip(state_hashes)
            .map(|(label, hash)| TrailStep {
                label: label.clone(),
                state_hash: hash,
                state_snapshot: None,
                source_loc: None,
            })
            .collect();

        Self {
            property_name: violation.property_name,
            description: violation.description,
            steps,
            states_explored,
            depth_reached,
        }
    }

    /// Save trail to a file in JSON format.
    pub fn save_json(&self, path: &Path) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, self)?;
        writer.flush()?;
        Ok(())
    }

    /// Load trail from a JSON file.
    pub fn load_json(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Save trail in Spin-compatible text format.
    pub fn save_spin_format(&self, path: &Path) -> std::io::Result<()> {
        let mut file = File::create(path)?;

        writeln!(file, "spin trail: {}", self.property_name)?;
        writeln!(file, "description: {}", self.description)?;
        writeln!(file, "states explored: {}", self.states_explored)?;
        writeln!(file, "depth reached: {}", self.depth_reached)?;
        writeln!(file)?;
        writeln!(file, "error trail:")?;

        for (i, step) in self.steps.iter().enumerate() {
            writeln!(
                file,
                "{:4}: {} (state hash: {:016x})",
                i + 1,
                step.label,
                step.state_hash
            )?;
        }

        Ok(())
    }

    /// Print trail to stdout in human-readable format.
    pub fn print(&self) {
        println!("\n=== Error Trail: {} ===", self.property_name);
        println!("Description: {}", self.description);
        println!("States explored: {}", self.states_explored);
        println!("Depth reached: {}", self.depth_reached);
        println!("\nTrail:");

        for (i, step) in self.steps.iter().enumerate() {
            println!(
                "  {:3}: {} [hash: {:08x}]",
                i + 1,
                step.label,
                step.state_hash & 0xFFFF_FFFF
            );
        }
    }

    /// Get the number of steps in the trail.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if the trail is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// Trail replay engine for step-by-step execution.
pub struct TrailReplayer<M> {
    model: M,
    trail: ErrorTrail,
}

impl<M: crate::engine::checker::Model> TrailReplayer<M> {
    /// Create a new trail replayer.
    pub fn new(model: M, trail: ErrorTrail) -> Self {
        Self { model, trail }
    }

    /// Replay the entire trail, returning states at each step.
    pub fn replay(&self) -> anyhow::Result<Vec<M::State>> {
        let mut states = Vec::new();
        let mut current_states = self.model.init_states();

        if current_states.is_empty() {
            return Ok(states);
        }

        // Find initial state matching first step's hash
        let mut current_state = None;
        for state in &current_states {
            if self.model.hash(state) == self.trail.steps.first().map(|s| s.state_hash).unwrap_or(0) {
                current_state = Some(state.clone());
                break;
            }
        }

        let mut state = current_state.unwrap_or_else(|| current_states.remove(0));
        states.push(state.clone());

        // Follow trail transitions
        for step in &self.trail.steps {
            let transitions = self.model.transitions(&state);
            
            // Find matching transition
            let mut found = false;
            for trans in &transitions {
                if trans.label == step.label {
                    state = trans.next.clone();
                    states.push(state.clone());
                    found = true;
                    break;
                }
            }

            if !found {
                // Transition not found - model may have changed
                anyhow::bail!(
                    "Trail replay failed: transition '{}' not found at step {}",
                    step.label,
                    states.len()
                );
            }
        }

        Ok(states)
    }

    /// Replay a single step, returning the next state.
    pub fn replay_step(&self, state: &M::State, step_idx: usize) -> anyhow::Result<M::State> {
        if step_idx >= self.trail.steps.len() {
            anyhow::bail!("Step index {} out of bounds (trail has {} steps)", step_idx, self.trail.steps.len());
        }

        let step = &self.trail.steps[step_idx];
        let transitions = self.model.transitions(state);

        for trans in &transitions {
            if trans.label == step.label {
                return Ok(trans.next.clone());
            }
        }

        anyhow::bail!("Transition '{}' not found at step {}", step.label, step_idx);
    }

    /// Get the trail being replayed.
    pub fn trail(&self) -> &ErrorTrail {
        &self.trail
    }
}

/// Trail statistics for reporting.
#[derive(Debug, Clone)]
pub struct TrailStats {
    /// Number of steps in the trail.
    pub trail_length: usize,
    /// Number of unique states visited.
    pub unique_states: usize,
    /// Number of process switches.
    pub process_switches: usize,
    /// Number of channel operations.
    pub channel_ops: usize,
    /// Number of assertion checks.
    pub assertion_checks: usize,
}

impl TrailStats {
    /// Compute statistics for a trail.
    pub fn compute(trail: &ErrorTrail) -> Self {
        let mut process_switches = 0;
        let mut channel_ops = 0;
        let mut assertion_checks = 0;
        let mut unique_hashes = std::collections::HashSet::new();

        let mut last_process: Option<String> = None;

        for step in &trail.steps {
            unique_hashes.insert(step.state_hash);

            // Extract process name from label (e.g., "P:1" -> "P")
            if let Some(colon_pos) = step.label.find(':') {
                let process = &step.label[..colon_pos];
                if let Some(last) = &last_process
                    && last != process {
                        process_switches += 1;
                    }
                last_process = Some(process.to_string());
            }

            // Count channel operations
            if step.label.contains('!') || step.label.contains('?') {
                channel_ops += 1;
            }

            // Count assertions
            if step.label.contains("assert") {
                assertion_checks += 1;
            }
        }

        Self {
            trail_length: trail.steps.len(),
            unique_states: unique_hashes.len(),
            process_switches,
            channel_ops,
            assertion_checks,
        }
    }

    /// Print statistics in Spin-compatible format.
    pub fn print_spin_format(&self) {
        println!("\n=== Trail Statistics ===");
        println!("trail length:          {}", self.trail_length);
        println!("unique states:         {}", self.unique_states);
        println!("process switches:      {}", self.process_switches);
        println!("channel operations:    {}", self.channel_ops);
        println!("assertion checks:      {}", self.assertion_checks);
    }
}

/// Generate a trail from a violation during model checking.
pub fn generate_trail<M: crate::engine::checker::Model>(
    _model: &M,
    violation: Violation,
    state_hashes: Vec<u64>,
    states_explored: usize,
    depth_reached: usize,
) -> ErrorTrail {
    ErrorTrail::new(violation, state_hashes, states_explored, depth_reached)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_trail() -> ErrorTrail {
        let violation = Violation {
            property_name: "assertion".to_string(),
            trail: vec!["P:x=1".into(), "Q:y=1".into(), "P:assert".into()],
            description: "assertion failed".to_string(),
        };
        let hashes = vec![0x1234, 0x5678, 0x9ABC, 0xDEF0];
        ErrorTrail::new(violation, hashes, 100, 3)
    }

    #[test]
    fn test_trail_creation() {
        let trail = sample_trail();
        assert_eq!(trail.len(), 3);
        assert_eq!(trail.property_name, "assertion");
    }

    #[test]
    fn test_trail_json_roundtrip() {
        let trail = sample_trail();
        let temp_path = std::env::temp_dir().join("test_trail.json");
        
        trail.save_json(&temp_path).unwrap();
        let loaded = ErrorTrail::load_json(&temp_path).unwrap();
        
        assert_eq!(loaded.len(), trail.len());
        assert_eq!(loaded.property_name, trail.property_name);
        
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_trail_spin_format() {
        let trail = sample_trail();
        let temp_path = std::env::temp_dir().join("test_trail.trail");
        
        trail.save_spin_format(&temp_path).unwrap();
        let content = std::fs::read_to_string(&temp_path).unwrap();
        
        assert!(content.contains("spin trail:"));
        assert!(content.contains("P:x=1"));
        
        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_trail_stats() {
        let trail = sample_trail();
        let stats = TrailStats::compute(&trail);
        
        assert_eq!(stats.trail_length, 3);
        assert_eq!(stats.unique_states, 3);
        assert_eq!(stats.assertion_checks, 1);
    }

    #[test]
    fn test_trail_replay() {
        use crate::engine::checker::{Model, Transition};

        struct TestModel;
        impl Model for TestModel {
            type State = u8;
            fn init_states(&self) -> Vec<u8> { vec![0] }
            fn transitions(&self, state: &u8) -> Vec<Transition<u8>> {
                match state {
                    0 => vec![Transition { label: "P:x=1".into(), next: 1 }],
                    1 => vec![Transition { label: "Q:y=1".into(), next: 2 }],
                    2 => vec![Transition { label: "P:assert".into(), next: 3 }],
                    _ => vec![],
                }
            }
            fn hash(&self, state: &u8) -> u64 { *state as u64 }
        }

        let model = TestModel;
        let violation = Violation {
            property_name: "test".into(),
            trail: vec!["P:x=1".into(), "Q:y=1".into(), "P:assert".into()],
            description: "test violation".into(),
        };
        let hashes = vec![0, 1, 2, 3];
        let trail = ErrorTrail::new(violation, hashes, 4, 3);
        
        let replayer = TrailReplayer::new(model, trail);
        let states = replayer.replay().unwrap();
        
        assert_eq!(states.len(), 4);
        assert_eq!(states, vec![0, 1, 2, 3]);
    }
}
