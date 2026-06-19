# Spin-Compatible Trail Format

## Overview

Implement Spin binary trail format (`.trail`) for compatibility with `spin -t` replay tool, while maintaining existing JSON and text formats for human readability.

## Requirements

### Functional Requirements

#### R1: Binary Format Support

- **R1.1**: Write trails in Spin binary format (reverse-engineered from Spin 6.5.x)
- **R1.2**: Include all required header fields (magic, version, states, depth, steps)
- **R1.3**: Encode per-step information (process ID, statement line, transition type, data)

#### R2: Format Selection

- **R2.1**: Support multiple output formats: JSON, Spin binary, Spin text
- **R2.2**: CLI flag to select format: `--trail-format json|binary|text`
- **R2.3**: Default to JSON for human readability

#### R3: Trail Content

- **R3.1**: Record all transitions from initial state to violation
- **R3.2**: Include process ID for each step
- **R3.3**: Include source location (file:line) when available
- **R3.4**: Include variable values at each step (for Spin binary format)

#### R4: Replay Support

- **R4.1**: Load trails from binary format
- **R4.2**: Replay trail step-by-step on the model
- **R4.3**: Verify trail validity (each step is a valid transition)

### Non-Functional Requirements

#### R5: Compatibility

- **R5.1**: Binary format compatible with `spin -t` (or document differences)
- **R5.2**: Handle endianness correctly (Spin uses little-endian)
- **R5.3**: Version field allows for future format evolution

#### R6: Performance

- **R6.1**: Trail writing overhead < 1ms per step
- **R6.2**: Trail file size < 1MB for typical violations (<1000 steps)
- **R6.3**: Minimal memory overhead during trail generation

## Spin Binary Format Specification

Based on reverse-engineering Spin 6.5.x source:

```
Header (24 bytes):
┌─────────────────────────────────────────────────────────────┐
│ Magic: 0x56455253 ("VERS")              │ 4 bytes           │
├─────────────────────────────────────────────────────────────┤
│ Version: uint32 (currently 2)           │ 4 bytes           │
├─────────────────────────────────────────────────────────────┤
│ States explored: uint64                 │ 8 bytes           │
├─────────────────────────────────────────────────────────────┤
│ Depth reached: uint32                   │ 4 bytes           │
├─────────────────────────────────────────────────────────────┤
│ Number of steps: uint32                 │ 4 bytes           │
└─────────────────────────────────────────────────────────────┘

Per Step (variable size):
┌─────────────────────────────────────────────────────────────┐
│ Process ID: uint16                      │ 2 bytes           │
├─────────────────────────────────────────────────────────────┤
│ Statement line: uint16                  │ 2 bytes           │
├─────────────────────────────────────────────────────────────┤
│ Transition type: uint8                  │ 1 byte            │
│   0 = normal                            │                   │
│   1 = send                              │                   │
│   2 = receive                           │                   │
│   3 = assignment                        │                   │
│   4 = assert                            │                   │
│   ...                                   │                   │
├─────────────────────────────────────────────────────────────┤
│ Optional data: variable-length          │ depends on type   │
│   - For send/receive: channel ID, message                 │
│   - For assignment: variable ID, value                    │
│   - For assert: assertion result                          │
└─────────────────────────────────────────────────────────────┘
```

**Note**: This is a best-effort reverse-engineering. Actual Spin format may differ. If incompatibility is discovered, we'll document it and provide a conversion tool.

## Data Structures

### Trail Format Enum

```rust
/// Trail output format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrailFormat {
    /// Human-readable JSON (v1 default)
    Json,
    /// Spin-compatible binary format
    SpinBinary,
    /// Human-readable text summary
    SpinText,
}

impl Default for TrailFormat {
    fn default() -> Self {
        TrailFormat::Json
    }
}
```

### Enhanced ErrorTrail

```rust
/// Complete error trail with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTrail {
    /// Property name that was violated
    pub property_name: String,
    /// Description of the violation
    pub description: String,
    /// Sequence of steps from initial state to violation
    pub steps: Vec<TrailStep>,
    /// Total states explored during verification
    pub states_explored: usize,
    /// Maximum depth reached
    pub depth_reached: usize,
    /// Promela source file (for trail replay)
    pub source_file: Option<String>,
}

/// A single step in an error trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailStep {
    /// Transition label (e.g., "P:1" for proctype P, line 1)
    pub label: String,
    /// State hash at this point
    pub state_hash: u64,
    /// Process ID (for Spin compatibility)
    pub process_id: Option<u16>,
    /// Statement line number (for Spin compatibility)
    pub line_number: Option<u16>,
    /// Transition type (for Spin binary format)
    pub transition_type: Option<u8>,
    /// Optional state snapshot (for debugging)
    pub state_snapshot: Option<String>,
    /// Source location (file:line) if available
    pub source_loc: Option<String>,
}

impl ErrorTrail {
    /// Save trail in specified format
    pub fn save(&self, path: &Path, format: TrailFormat) -> io::Result<()> {
        match format {
            TrailFormat::Json => self.save_json(path),
            TrailFormat::SpinBinary => self.save_spin_binary(path),
            TrailFormat::SpinText => self.save_spin_text(path),
        }
    }
    
    /// Save in Spin binary format
    pub fn save_spin_binary(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        
        // Write header
        file.write_all(&0x56455253u32.to_le_bytes())?; // Magic "VERS"
        file.write_all(&2u32.to_le_bytes())?;          // Version
        file.write_all(&(self.states_explored as u64).to_le_bytes())?;
        file.write_all(&(self.depth_reached as u32).to_le_bytes())?;
        file.write_all(&(self.steps.len() as u32).to_le_bytes())?;
        
        // Write steps
        for step in &self.steps {
            // Parse process ID and line from label
            let (pid, line) = self.parse_spin_label(&step.label)?;
            
            file.write_all(&pid.to_le_bytes())?;
            file.write_all(&line.to_le_bytes())?;
            
            // Determine transition type
            let trans_type = self.determine_transition_type(&step.label);
            file.write_all(&[trans_type])?;
            
            // Optional data (minimal for v2)
            // Full implementation would encode variable values
        }
        
        Ok(())
    }
    
    /// Parse Spin label format "P:line" or "P_id:line"
    fn parse_spin_label(&self, label: &str) -> io::Result<(u16, u16)> {
        // Expected format: "proctype_name:statement_line"
        // or "proctype_name_pid:statement_line"
        
        if let Some(colon_pos) = label.rfind(':') {
            let _proc_part = &label[..colon_pos];
            let line_part = &label[colon_pos + 1..];
            
            let line: u16 = line_part.parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid line number"))?;
            
            // Extract process ID from proctype name if present
            // Simplified: assume PID 0 for now
            let pid = 0;
            
            Ok((pid, line))
        } else {
            // No colon, use defaults
            Ok((0, 0))
        }
    }
    
    /// Determine transition type from label
    fn determine_transition_type(&self, label: &str) -> u8 {
        if label.contains('!') {
            1 // send
        } else if label.contains('?') {
            2 // receive
        } else if label.contains('=') {
            3 // assignment
        } else if label.contains("assert") {
            4 // assert
        } else {
            0 // normal
        }
    }
    
    /// Load trail from Spin binary format
    pub fn load_spin_binary(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        
        // Read header
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic)?;
        if &magic != b"VERS" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid magic number"));
        }
        
        let mut version = [0u8; 4];
        file.read_exact(&mut version)?;
        let version = u32::from_le_bytes(version);
        
        if version != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported trail format version: {}", version)
            ));
        }
        
        let mut states_buf = [0u8; 8];
        file.read_exact(&mut states_buf)?;
        let states_explored = u64::from_le_bytes(states_buf) as usize;
        
        let mut depth_buf = [0u8; 4];
        file.read_exact(&mut depth_buf)?;
        let depth_reached = u32::from_le_bytes(depth_buf) as usize;
        
        let mut steps_buf = [0u8; 4];
        file.read_exact(&mut steps_buf)?;
        let num_steps = u32::from_le_bytes(steps_buf) as usize;
        
        // Read steps
        let mut steps = Vec::with_capacity(num_steps);
        for _ in 0..num_steps {
            let mut pid_buf = [0u8; 2];
            file.read_exact(&mut pid_buf)?;
            let _pid = u16::from_le_bytes(pid_buf);
            
            let mut line_buf = [0u8; 2];
            file.read_exact(&mut line_buf)?;
            let line = u16::from_le_bytes(line_buf);
            
            let mut type_buf = [0u8; 1];
            file.read_exact(&mut type_buf)?;
            let _trans_type = type_buf[0];
            
            // Construct step
            steps.push(TrailStep {
                label: format!("P:{}", line),
                state_hash: 0, // Not stored in binary format
                process_id: Some(_pid),
                line_number: Some(line),
                transition_type: Some(_trans_type),
                state_snapshot: None,
                source_loc: None,
            });
        }
        
        Ok(Self {
            property_name: "loaded".to_string(),
            description: "Loaded from Spin binary trail".to_string(),
            steps,
            states_explored,
            depth_reached,
            source_file: None,
        })
    }
}
```

### Trail Replayer (Enhanced)

```rust
impl<M: crate::engine::checker::Model> TrailReplayer<M> {
    /// Replay trail loaded from Spin binary format
    pub fn replay_spin_trail(&self, trail_path: &Path) -> anyhow::Result<Vec<M::State>> {
        let trail = ErrorTrail::load_spin_binary(trail_path)?;
        
        // Verify trail matches current model
        // (simplified: just check if we can replay)
        self.replay_trail(&trail)
    }
    
    fn replay_trail(&self, trail: &ErrorTrail) -> anyhow::Result<Vec<M::State>> {
        let mut states = Vec::new();
        let mut current_states = self.model.init_states();
        
        if current_states.is_empty() {
            return Ok(states);
        }
        
        let mut state = current_states.remove(0);
        states.push(state.clone());
        
        // Follow trail transitions
        for step in &trail.steps {
            let transitions = self.model.transitions(&state);
            
            // Find matching transition
            let mut found = false;
            for trans in &transitions {
                if self.transition_matches(&trans, step) {
                    state = trans.next.clone();
                    states.push(state.clone());
                    found = true;
                    break;
                }
            }
            
            if !found {
                anyhow::bail!(
                    "Trail replay failed: transition '{}' not found at step {}",
                    step.label,
                    states.len()
                );
            }
        }
        
        Ok(states)
    }
    
    fn transition_matches(&self, trans: &Transition<M::State>, step: &TrailStep) -> bool {
        // Match by label or by process ID + line
        if trans.label == step.label {
            return true;
        }
        
        // Try matching by process ID and line
        if let (Some(pid), Some(line)) = (step.process_id, step.line_number) {
            let label = format!("P:{}", line);
            if trans.label == label {
                return true;
            }
        }
        
        false
    }
}
```

## Interface

### CLI

```bash
# Save trail in JSON format (default)
spin-rs --trail-file error.json model.pml

# Save trail in Spin binary format
spin-rs --trail-file error.trail --trail-format binary model.pml

# Save trail in Spin text format
spin-rs --trail-file error.txt --trail-format text model.pml

# Replay trail with spin -t (if format is compatible)
spin -t model.pml error.trail

# Replay trail with spin-rs
spin-rs --replay error.trail model.pml
```

### Library API

```rust
use spin_rs::{verify, trail::{ErrorTrail, TrailFormat}};
use std::path::Path;

let result = verify(promela)?;

if let Some(violation) = result.violations.first() {
    let trail = ErrorTrail::new(
        violation.clone(),
        vec![],
        result.states_explored,
        result.depth_reached,
    );
    
    // Save in multiple formats
    trail.save(Path::new("error.json"), TrailFormat::Json)?;
    trail.save(Path::new("error.trail"), TrailFormat::SpinBinary)?;
    trail.save(Path::new("error.txt"), TrailFormat::SpinText)?;
}
```

## Testing

### Unit Tests

```rust
#[test]
fn test_spin_binary_format_roundtrip() {
    let trail = create_test_trail();
    let temp_path = std::env::temp_dir().join("test.trail");
    
    // Save in binary format
    trail.save_spin_binary(&temp_path).unwrap();
    
    // Load back
    let loaded = ErrorTrail::load_spin_binary(&temp_path).unwrap();
    
    // Verify content
    assert_eq!(loaded.steps.len(), trail.steps.len());
    assert_eq!(loaded.states_explored, trail.states_explored);
    
    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_label_parsing() {
    let trail = ErrorTrail { /* ... */ };
    
    let (pid, line) = trail.parse_spin_label("P:5").unwrap();
    assert_eq!(line, 5);
    
    let (pid, line) = trail.parse_spin_label("Worker_0:12").unwrap();
    assert_eq!(line, 12);
}

#[test]
fn test_transition_type_detection() {
    let trail = ErrorTrail { /* ... */ };
    
    assert_eq!(trail.determine_transition_type("q!1"), 1); // send
    assert_eq!(trail.determine_transition_type("q?x"), 2); // receive
    assert_eq!(trail.determine_transition_type("x=1"), 3); // assignment
    assert_eq!(trail.determine_transition_type("assert(x)"), 4); // assert
    assert_eq!(trail.determine_transition_type("skip"), 0); // normal
}
```

### Integration Tests

```rust
#[test]
fn test_trail_replay() {
    let promela = r#"
        active proctype P() {
            byte x = 0;
            x = 1;
            x = 2;
            assert(x == 3); // Will fail
        }
    "#;
    
    let result = verify(promela).unwrap();
    assert!(result.errors > 0);
    
    let violation = &result.violations[0];
    let trail = ErrorTrail::new(
        violation.clone(),
        vec![],
        result.states_explored,
        result.depth_reached,
    );
    
    // Save and reload
    let temp_path = std::env::temp_dir().join("replay.trail");
    trail.save_spin_binary(&temp_path).unwrap();
    
    let model = LuaModel::from_source(promela).unwrap();
    let replayer = TrailReplayer::new(model, trail);
    
    // Replay should succeed
    let states = replayer.replay_spin_trail(&temp_path).unwrap();
    assert!(states.len() > 0);
    
    std::fs::remove_file(temp_path).ok();
}
```

## Dependencies

- Existing `trail` module (v1)
- Standard library file I/O
- No new external dependencies

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Binary format incompatibility | `spin -t` can't replay | Document differences; provide conversion tool |
| Reverse-engineering errors | Corrupt trail files | Test thoroughly; validate with hex dump |
| Endianness issues | Cross-platform incompatibility | Always use little-endian; test on multiple architectures |
| Variable encoding complexity | Large trail files | Start with minimal encoding; enhance later |

## Success Criteria

- ✅ Binary format loads in Spin (or documented incompatibility)
- ✅ Trail replay works for all violation types
- ✅ File size < 100KB for typical trails (<100 steps)
- ✅ CLI supports `--trail-format` flag
- ✅ JSON format remains default (backward compatible)
