# POR C3 Cycle Condition

## Overview

Implement the C3 cycle condition for partial order reduction to ensure soundness. Without C3, POR can miss violations by pruning transitions that lead to accepting cycles.

## Requirements

### Functional Requirements

#### R1: Cycle Detection

- **R1.1**: Detect when the DFS stack contains a cycle (state repeated on stack)
- **R1.2**: Identify all states in the cycle
- **R1.3**: Track which transitions have been expanded at each state in the cycle

#### R2: C3 Condition

- **R2.1**: If any state in the cycle has unexpanded transitions, force full expansion at some state in the cycle
- **R2.2**: Conservative approach: expand all transitions at the current state if C3 violated
- **R2.3**: C3 check performed at every state before applying POR

#### R3: Sound Ample Set

- **R3.1**: Compute ample set only if C3 condition satisfied
- **R3.2**: If C3 violated, return all enabled transitions (no reduction)
- **R3.3**: Preserve C0-C2 conditions (non-empty, invisible, independence)

#### R4: Integration

- **R4.1**: Integrate with existing `PorManager` infrastructure
- **R4.2**: Work with both DFS and BFS (though C3 primarily for DFS)
- **R4.3**: Support nested DFS for LTL (C3 applies to both outer and inner DFS)

### Non-Functional Requirements

#### R5: Performance

- **R5.1**: C3 check overhead < 1μs per state
- **R5.2**: Stack hashing efficient (cached hashes)
- **R5.3**: No allocation in hot path

#### R6: Correctness

- **R6.1**: Never prune transitions when C3 violated
- **R6.2**: Preserve all accepting cycles in reduced state space
- **R6.3**: Match Spin's C3 behavior (for comparison testing)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    C3 Cycle Condition Architecture                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  DFS Stack (current path)                                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ s₀ (initial)                                                │   │
│  │  │                                                          │   │
│  │  ▼                                                          │   │
│  │ s₁                                                          │   │
│  │  │                                                          │   │
│  │  ▼                                                          │   │
│  │ s₂  ◄─────────────────────────────────────┐                │   │
│  │  │                                        │                │   │
│  │  ▼                                        │                │   │
│  │ s₃                                        │                │   │
│  │  │                                        │                │   │
│  │  ▼                                        │                │   │
│  │ s₄ (current) ─── Transition to s₂ ────────┘                │   │
│  │                                                            │   │
│  │  Cycle detected: s₂ → s₃ → s₄ → s₂                         │   │
│  │                                                            │   │
│  │  C3 Check:                                                 │   │
│  │  - For each state in cycle (s₂, s₃, s₄):                  │   │
│  │    - Were all transitions expanded?                        │   │
│  │    - If NO: C3 violated!                                   │   │
│  │                                                            │   │
│  │  If C3 violated:                                           │   │
│  │  → Expand ALL transitions at current state (s₄)           │   │
│  │  → No POR at this state                                    │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Structures

### Enhanced PorManager

```rust
pub struct PorManager<S> {
    /// Cached dependency info per state hash
    deps_cache: HashMap<u64, Vec<TransitionDeps>>,
    
    /// C3 cycle detection: track expanded transitions per state on stack
    /// Key: state hash, Value: set of expanded transition indices
    expanded_on_stack: HashMap<u64, HashSet<usize>>,
    
    _marker: std::marker::PhantomData<S>,
}

impl<S: Clone + Hash + Eq + Send> PorManager<S> {
    /// Compute ample set with C3 condition
    pub fn compute_ample_set_with_c3<M: Model<State = S>>(
        &mut self,
        model: &M,
        state: &S,
        transitions: &[Transition<S>],
        stack: &[S],  // Current DFS path
    ) -> Vec<usize> {
        // Step 1: Compute dependency info
        let deps = self.analyze(model, state, transitions);
        
        // Step 2: Check C3 condition
        let c3_violated = self.check_c3(state, stack, transitions);
        
        // Step 3: If C3 violated, return all transitions (no POR)
        if c3_violated {
            return (0..transitions.len()).collect();
        }
        
        // Step 4: Compute persistent set (C0-C2)
        let ample = self.compute_persistent_set(&deps);
        
        // Step 5: Mark these transitions as expanded for C3
        let state_hash = model.hash(state);
        let expanded = self.expanded_on_stack
            .entry(state_hash)
            .or_insert_with(HashSet::new);
        expanded.extend(&ample);
        
        ample
    }
    
    /// Check C3 cycle condition
    fn check_c3(
        &self,
        state: &S,
        stack: &[S],
        transitions: &[Transition<S>],
    ) -> bool {
        let state_hash = hash(state);
        
        // Find if this state is on the stack (cycle detection)
        if let Some(cycle_start) = stack.iter()
            .position(|s| hash(s) == state_hash)
        {
            // Cycle detected! Check if all states in cycle have all transitions expanded
            let cycle = &stack[cycle_start..];
            
            for cycle_state in cycle {
                let cycle_hash = hash(cycle_state);
                let expanded = self.expanded_on_stack.get(&cycle_hash);
                
                // If any state in cycle has unexpanded transitions, C3 violated
                if let Some(expanded_set) = expanded {
                    if expanded_set.len() < transitions.len() {
                        return true; // C3 violated
                    }
                } else {
                    // State never had transitions expanded? C3 violated
                    return true;
                }
            }
        }
        
        false // C3 satisfied
    }
    
    /// Standard persistent set computation (C0-C2)
    fn compute_persistent_set(
        &self,
        deps: &[TransitionDeps],
    ) -> Vec<usize> {
        // C0: Non-empty
        if deps.is_empty() {
            return vec![];
        }
        
        // C1: If any visible, return all
        if deps.iter().any(|d| d.visible) {
            return (0..deps.len()).collect();
        }
        
        // C2: Try to find singleton ample set (local, independent)
        for (i, dep) in deps.iter().enumerate() {
            if dep.local && !dep.visible {
                let independent = deps.iter().enumerate()
                    .filter(|&(j, _)| i != j)
                    .all(|(_, other)| self.are_independent(dep, other));
                
                if independent {
                    return vec![i];
                }
            }
        }
        
        // Fallback: all transitions
        (0..deps.len()).collect()
    }
}
```

### DFS with C3

```rust
/// DFS with POR and C3 condition
pub fn check_dfs_por_c3<M: Model>(
    model: &M,
    max_states: usize,
    max_depth: usize,
) -> CheckResult {
    let start = std::time::Instant::now();
    let init_states = model.init_states();

    if init_states.is_empty() {
        return empty_result(0.0);
    }

    let mut por_manager = PorManager::new();
    let mut visited: HashSet<u64> = HashSet::new();
    let mut stack: Vec<(M::State, usize)> = Vec::new();
    let mut dfs_path: Vec<M::State> = Vec::new();  // For C3
    let mut transitions_count = 0;
    let mut max_depth_reached = 0;

    for s in init_states {
        let h = model.hash(&s);
        if visited.insert(h) {
            stack.push((s, 0));
            dfs_path.push(s.clone());
        }
    }

    while let Some((state, depth)) = stack.pop() {
        max_depth_reached = max_depth_reached.max(depth);

        if depth >= max_depth {
            dfs_path.pop();
            continue;
        }
        if visited.len() >= max_states {
            break;
        }

        let all_transitions = model.transitions(&state);
        
        // Apply POR with C3
        let ample_indices = por_manager.compute_ample_set_with_c3(
            model,
            &state,
            &all_transitions,
            &dfs_path,
        );
        
        // Explore only ample set
        let transitions_to_explore: Vec<_> = ample_indices
            .iter()
            .map(|&i| &all_transitions[i])
            .collect();
        
        transitions_count += transitions_to_explore.len();

        for t in transitions_to_explore {
            let h = model.hash(&t.next);
            if visited.insert(h) {
                stack.push((t.next.clone(), depth + 1));
                dfs_path.push(t.next.clone());
            }
        }
        
        // Pop from DFS path when backtracking
        // (simplified: in real impl, track more carefully)
        dfs_path.pop();
    }

    let elapsed = start.elapsed().as_secs_f64();

    CheckResult {
        states_explored: visited.len(),
        states_stored: visited.len(),
        transitions: transitions_count,
        depth_reached: max_depth_reached,
        errors: 0,
        violations: vec![],
        elapsed_secs: elapsed,
    }
}
```

## C3 Correctness Proof Sketch

**Theorem**: If an accepting cycle exists in the full state space, it exists in the reduced state space constructed with C3.

**Proof sketch**:

1. Assume accepting cycle C exists in full space: s₀ → s₁ → ... → sₖ → s₀
2. Suppose C is not in reduced space. Then some transition in C was pruned.
3. Let (sᵢ, sᵢ₊₁) be the first pruned transition in C.
4. At state sᵢ, POR pruned the transition to sᵢ₊₁.
5. By C3 condition: if sᵢ is in a cycle on the DFS stack, and any state in that cycle has unexpanded transitions, we expand all at sᵢ.
6. Since C is a cycle, sᵢ will eventually be on the stack with the rest of C.
7. If the transition (sᵢ, sᵢ₊₁) was pruned, C3 must have been satisfied (all states in cycle had all transitions expanded).
8. Contradiction: if all transitions were expanded at all states in C, the cycle would be in the reduced space.
9. Therefore, C is in the reduced space. ∎

## Testing

### Unit Tests

```rust
#[test]
fn test_c3_cycle_detection() {
    // Create a model with a known cycle
    let model = CycleModel::new(3); // 3-state cycle
    let mut por_manager = PorManager::new();
    
    // Simulate DFS path: s₀ → s₁ → s₂ → s₀ (cycle)
    let stack = vec![model.state(0), model.state(1), model.state(2)];
    let current = model.state(0); // Back to start
    
    // C3 should detect the cycle
    let transitions = model.transitions(&current);
    let c3_violated = por_manager.check_c3(&current, &stack, &transitions);
    
    // Depends on whether transitions were expanded
    // ...
}

#[test]
fn test_c3_forces_expansion() {
    // Model where POR would prune, but C3 forces expansion
    let model = PorCycleModel::new();
    let mut por_manager = PorManager::new();
    
    // Set up state where C3 is violated
    // ...
    
    let ample = por_manager.compute_ample_set_with_c3(
        &model,
        &state,
        &transitions,
        &stack,
    );
    
    // Should return all transitions (no pruning)
    assert_eq!(ample.len(), transitions.len());
}
```

### Integration Tests

```rust
#[test]
fn test_por_c3_preserves_liveness() {
    // Model with liveness violation that POR without C3 would miss
    let promela = r#"
        byte x = 0;
        active proctype P() {
            do
            :: x == 0 -> x = 1
            :: x == 1 -> x = 0
            od
        }
    "#;
    
    // LTL: []<>(x == 0) - should hold
    // POR without C3 might prune the x=0 transition
    // POR with C3 must preserve the cycle
    
    let result = verify_ltl_with_por(promela, "[]<>(x == 0)", true).unwrap();
    assert!(result.is_none()); // Property holds
}
```

### Comparison with Spin

```rust
#[test]
fn test_por_reduction_matches_spin() {
    // Run same model through Spin and spin-rs with POR
    // Compare state counts (should be similar reduction ratio)
    
    let promela = include_str!("../examples/peterson.pml");
    
    let spin_states = run_spin_with_por(promela);
    let spin_rs_states = run_spin_rs_with_por_c3(promela);
    
    // Reduction ratio should be similar (within 20%)
    let spin_reduction = spin_states.full as f64 / spin_states.por as f64;
    let spin_rs_reduction = spin_rs_states.full as f64 / spin_rs_states.por as f64;
    
    assert!((spin_reduction - spin_rs_reduction).abs() < 0.2 * spin_reduction);
}
```

## Dependencies

- Existing `PorManager` infrastructure (v1)
- DFS stack tracking (enhanced from v1)
- Transition dependency analysis (enhanced from v1)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| C3 check too slow | Performance degradation | Optimize stack hashing; cache cycle detection |
| False cycle detection | Over-expansion (no POR) | Careful stack management; test thoroughly |
| Missing cycles | Unsound verification | Property-based testing; compare with Spin |
| Interaction with LTL | Complex nested DFS | Test with LTL properties; ensure C3 in both DFS levels |

## Success Criteria

- ✅ C3 condition correctly detects cycles
- ✅ C3 forces full expansion when violated
- ✅ No accepting cycles missed due to POR
- ✅ Performance overhead < 10% vs POR without C3
- ✅ State-space reduction similar to Spin's POR
