# LTL Parser Specification

## Overview

Parse LTL formulas from strings into an AST, supporting standard operators with clear error messages for unsupported features.

## Requirements

### Functional Requirements

### Requirement: Operator Support

The parser SHALL ALWAYS support the following operators:

- **Temporal operators**: The parser MUST ALWAYS parse `[]` (always/globally), `<>` (eventually/finally), and `X` (next) operators. [R1.1, R1.2]
- **Boolean operators**: The parser MUST ALWAYS parse `&&` (and), `||` (or), `!` (not), and `->` (implies) operators. [R1.2]
- **Atomic propositions**: The parser MUST ALWAYS parse variable comparisons such as `x == 0`, `y > 1`, and boolean variables like `flag`. [R1.3]
- **Parentheses**: The parser MUST ALWAYS support parentheses for grouping, e.g., `(p && q) || r`. [R1.4]
- **Spin syntax aliases**: The parser SHOULD ALWAYS support Spin syntax aliases: `G` (globally), `F` (finally), `O` (next). [R1.5]

### Requirement: Error Handling

The parser SHALL ALWAYS handle errors as follows:

- **Unsupported operators**: IF the input contains `U` (until) or `V` (release) operators, THEN the parser MUST return `LtlError::UnsupportedOperator`. [R2.1]
- **Nested temporal**: IF the input contains nested temporal operators such as `[]<>p` or `<>(p U q)`, THEN the parser MUST return `LtlError::NestedTemporal`. [R2.2]
- **Syntax errors**: IF the input has syntax errors, THEN the parser MUST return `LtlError::ParseError` with a helpful message. [R2.3]
- **Error detail**: Error messages SHALL ALWAYS include the problematic substring and its position in the input. [R2.4]

### Requirement: AST Output

The parser SHALL ALWAYS produce the following output:

- **LtlFormula enum**: The parser MUST ALWAYS produce an `LtlFormula` enum with variants for each operator (True, False, Atom, Not, And, Or, Always, Eventually, Next). [R3.1]
- **Implication normalization**: The parser MUST ALWAYS normalize `p -> q` to `!p || q` during parsing. [R3.2]
- **Source location**: The parser SHOULD preserve source location information for error reporting. [R3.3]

### Requirement: Performance

The parser SHALL ALWAYS meet the following performance requirements:

- **Parse time**: The parser MUST parse typical formulas (<10 operators) in <100μs. [R4.1]
- **Allocations**: The parser SHOULD use zero allocations for atomic propositions. [R4.2]
- **Formula size**: The parser MUST support formulas up to 1000 characters. [R4.3]

### Requirement: Usability

The parser SHALL ALWAYS provide the following usability features:

- **Actionable errors**: Error messages SHALL ALWAYS be actionable and suggest workarounds where possible. [R5.1]
- **Whitespace flexibility**: The parser MUST ALWAYS support flexible whitespace, treating `[] (p && q)` as equivalent to `[](p&&q)`. [R5.2]

## Interface

```rust
/// Parse LTL formula from string
pub fn parse_ltl(input: &str) -> Result<LtlFormula, LtlError>;

/// LTL formula AST
pub enum LtlFormula {
    True,
    False,
    Atom(String),
    Not(Box<LtlFormula>),
    And(Box<LtlFormula>, Box<LtlFormula>),
    Or(Box<LtlFormula>, Box<LtlFormula>),
    Always(Box<LtlFormula>),      // []
    Eventually(Box<LtlFormula>),  // <>
    Next(Box<LtlFormula>),        // X
}

/// LTL parsing errors
pub enum LtlError {
    UnsupportedOperator { op: String, suggestion: Option<String> },
    NestedTemporal { formula: String },
    ParseError { message: String, position: usize },
}
```

## Examples

### Valid Formulas

```rust
parse_ltl("[]p")           // Ok(Always(Atom("p")))
parse_ltl("<>(x == 0)")    // Ok(Eventually(Atom("x == 0")))
parse_ltl("X(flag)")       // Ok(Next(Atom("flag")))
parse_ltl("p && q")        // Ok(And(Atom("p"), Atom("q")))
parse_ltl("!(x > 0)")      // Ok(Not(Atom("x > 0")))
parse_ltl("p -> q")        // Ok(Or(Not(Atom("p")), Atom("q")))
parse_ltl("G(p || q)")     // Ok(Always(Or(Atom("p"), Atom("q"))))
```

### Invalid Formulas

```rust
parse_ltl("p U q")         // Err(UnsupportedOperator { op: "U", suggestion: Some("Use full ltl2ba implementation") })
parse_ltl("[]<>p")         // Err(NestedTemporal { formula: "[]<>p" })
parse_ltl("[](p V q)")     // Err(UnsupportedOperator { op: "V", suggestion: None })
parse_ltl("[]p q")         // Err(ParseError { message: "Unexpected token", position: 3 })
```

## Testing

### Unit Tests

- Parse each supported operator individually
- Parse nested boolean combinations: `(p && q) || (r && s)`
- Parse with various whitespace: `[] ( p )`, `[](p)`
- Error cases: unsupported operators, nested temporal, syntax errors

### Integration Tests

- Parse formulas from Spin model files
- Round-trip: parse → to_string → parse (should produce equivalent formula)
- Performance: parse 1000 formulas in <100ms

## Dependencies

- None (pure Rust parser, no external crates)

## Success Criteria

- ✅ Parse all formulas in "Valid Formulas" examples correctly
- ✅ Reject all formulas in "Invalid Formulas" examples with appropriate errors
- ✅ Error messages are actionable and include position information
- ✅ Performance meets R4 requirements
