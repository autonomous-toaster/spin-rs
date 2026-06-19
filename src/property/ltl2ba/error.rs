//! LTL parsing and conversion errors.

use std::fmt;

/// LTL parsing and conversion errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LtlError {
    /// Unsupported operator (e.g., U, V, nested temporal)
    UnsupportedOperator {
        /// The unsupported operator
        op: String,
        /// Suggested workaround (if any)
        suggestion: Option<String>,
    },

    /// Nested temporal operator (e.g., []<>p)
    NestedTemporal {
        /// The problematic formula
        formula: String,
    },

    /// Parse error (syntax error)
    ParseError {
        /// Error message
        message: String,
        /// Position in the input string (byte offset)
        position: usize,
    },

    /// Invalid atomic proposition
    InvalidAtom {
        /// The invalid atomic proposition
        atom: String,
    },
}

impl fmt::Display for LtlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LtlError::UnsupportedOperator { op, suggestion } => {
                write!(f, "Unsupported operator '{}'", op)?;
                if let Some(s) = suggestion {
                    write!(f, ": {}", s)?;
                }
                Ok(())
            }
            LtlError::NestedTemporal { formula } => {
                write!(
                    f,
                    "Nested temporal operators are not supported: '{}'",
                    formula
                )
            }
            LtlError::ParseError { message, position } => {
                write!(f, "Parse error at position {}: {}", position, message)
            }
            LtlError::InvalidAtom { atom } => {
                write!(f, "Invalid atomic proposition: '{}'", atom)
            }
        }
    }
}

impl std::error::Error for LtlError {}

impl LtlError {
    /// Create an UnsupportedOperator error with a suggestion.
    pub fn unsupported(op: impl Into<String>, suggestion: Option<&str>) -> Self {
        Self::UnsupportedOperator {
            op: op.into(),
            suggestion: suggestion.map(String::from),
        }
    }

    /// Create a NestedTemporal error.
    pub fn nested_temporal(formula: impl Into<String>) -> Self {
        Self::NestedTemporal {
            formula: formula.into(),
        }
    }

    /// Create a ParseError.
    pub fn parse_error(message: impl Into<String>, position: usize) -> Self {
        Self::ParseError {
            message: message.into(),
            position,
        }
    }

    /// Create an InvalidAtom error.
    pub fn invalid_atom(atom: impl Into<String>) -> Self {
        Self::InvalidAtom { atom: atom.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_unsupported() {
        let err = LtlError::unsupported("U", Some("Use full ltl2ba implementation"));
        assert!(err.to_string().contains("Unsupported operator 'U'"));
    }

    #[test]
    fn test_error_display_nested() {
        let err = LtlError::nested_temporal("[]<>p");
        assert!(err
            .to_string()
            .contains("Nested temporal operators are not supported"));
        assert!(err.to_string().contains("[]<>p"));
    }

    #[test]
    fn test_error_display_parse() {
        let err = LtlError::parse_error("Unexpected token", 5);
        assert!(err.to_string().contains("Parse error at position 5"));
        assert!(err.to_string().contains("Unexpected token"));
    }

    #[test]
    fn test_error_display_invalid_atom() {
        let err = LtlError::invalid_atom("x = 0");
        assert!(err.to_string().contains("Invalid atomic proposition"));
    }
}
