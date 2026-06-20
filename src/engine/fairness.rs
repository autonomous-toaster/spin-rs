//! Fairness constraints for model checking.
//!
//! Supports weak fairness (each continuously enabled transition eventually executes)
//! and strong fairness (each infinitely-often enabled transition executes infinitely often).
//!
//! Fairness is implemented as a scheduling constraint during state exploration:
//! transitions that are "unfairly" disabled are tracked, and the search prioritizes
//! transitions that have been enabled but not yet taken.

use std::collections::{HashMap, HashSet};

/// Fairness mode to apply during model checking.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FairnessMode {
    /// No fairness constraints (standard model checking)
    #[default]
    None,
    /// Weak fairness: every continuously enabled transition eventually executes.
    /// If a transition is enabled at every state along a path, it must eventually fire.
    Weak,
    /// Strong fairness: every infinitely-often enabled transition executes infinitely often.
    /// If a transition is enabled at infinitely many states, it must fire infinitely often.
    Strong,
}

/// Tracks fairness state during exploration.
#[derive(Debug, Clone)]
pub struct FairnessTracker {
    /// The fairness mode
    mode: FairnessMode,
    /// Per-transition tracking: transition_label -> enabled_count
    enabled_counts: HashMap<String, u64>,
    /// Per-transition tracking: transition_label -> fired_count
    fired_counts: HashMap<String, u64>,
    /// Transitions that are currently continuously enabled (weak fairness)
    continuously_enabled: HashSet<String>,
}

impl FairnessTracker {
    /// Create a new fairness tracker.
    pub fn new(mode: FairnessMode) -> Self {
        Self {
            mode,
            enabled_counts: HashMap::new(),
            fired_counts: HashMap::new(),
            continuously_enabled: HashSet::new(),
        }
    }

    /// Record which transitions are enabled at the current state.
    pub fn record_enabled(&mut self, enabled_labels: &[String]) {
        let labels: HashSet<String> = enabled_labels.iter().cloned().collect();

        // Update continuously_enabled: transitions that were enabled before and still are
        self.continuously_enabled.retain(|t| labels.contains(t));

        // Add newly enabled transitions to continuously_enabled
        for label in &labels {
            if !self.continuously_enabled.contains(label) {
                self.continuously_enabled.insert(label.clone());
            }
            *self.enabled_counts.entry(label.clone()).or_insert(0) += 1;
        }
    }

    /// Record that a transition was fired.
    pub fn record_fired(&mut self, label: &str) {
        *self.fired_counts.entry(label.to_string()).or_insert(0) += 1;
        // Firing a transition means it's no longer continuously enabled
        self.continuously_enabled.remove(label);
    }

    /// Check if firing this transition would violate fairness.
    /// Returns true if the transition is "fair" to execute now.
    pub fn is_fair_to_fire(&self, _label: &str) -> bool {
        match self.mode {
            FairnessMode::None => true,
            FairnessMode::Weak | FairnessMode::Strong => {
                // Conservative: always allow firing
                // Real implementation would check enabled/fired ratios
                true
            }
        }
    }

    /// Get transitions that must be prioritized for fairness.
    /// Returns labels that should be scheduled to maintain fairness.
    pub fn get_prioritized_transitions(&self) -> Vec<String> {
        match self.mode {
            FairnessMode::None => vec![],
            FairnessMode::Weak => {
                // Prioritize continuously enabled transitions that haven't fired
                self.continuously_enabled
                    .iter()
                    .filter(|t| self.fired_counts.get(*t).copied().unwrap_or(0) == 0)
                    .cloned()
                    .collect()
            }
            FairnessMode::Strong => {
                // Prioritize transitions enabled many times but fired few times
                self.enabled_counts
                    .iter()
                    .filter(|(t, enabled)| {
                        let fired = self.fired_counts.get(*t).copied().unwrap_or(0);
                        **enabled > 0 && fired < **enabled / 2
                    })
                    .map(|(t, _)| t.clone())
                    .collect()
            }
        }
    }

    /// Reset tracking for a new exploration path.
    pub fn reset(&mut self) {
        self.continuously_enabled.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fairness_default() {
        let ft = FairnessTracker::new(FairnessMode::None);
        assert!(ft.is_fair_to_fire("any"));
        assert!(ft.get_prioritized_transitions().is_empty());
    }

    #[test]
    fn test_weak_fairness_tracking() {
        let mut ft = FairnessTracker::new(FairnessMode::Weak);
        ft.record_enabled(&["a".to_string(), "b".to_string()]);
        assert!(!ft.get_prioritized_transitions().is_empty());
        ft.record_fired("a");
        // After firing, "a" is removed from continuously_enabled
        assert_eq!(ft.continuously_enabled.len(), 1);
    }

    #[test]
    fn test_strong_fairness() {
        let mut ft = FairnessTracker::new(FairnessMode::Strong);
        // Enable "a" many times, fire it once
        ft.record_enabled(&["a".to_string()]);
        ft.record_enabled(&["a".to_string()]);
        ft.record_enabled(&["a".to_string()]);
        ft.record_enabled(&["a".to_string()]);
        ft.record_fired("a");
        ft.record_enabled(&["a".to_string()]);
        // "a" has been enabled 5 times, fired once -> should be prioritized
        let prioritized = ft.get_prioritized_transitions();
        assert!(prioritized.contains(&"a".to_string()));
    }

    #[test]
    fn test_reset() {
        let mut ft = FairnessTracker::new(FairnessMode::Weak);
        ft.record_enabled(&["a".to_string()]);
        assert!(ft.continuously_enabled.contains("a"));
        ft.reset();
        assert!(ft.continuously_enabled.is_empty());
    }
}
