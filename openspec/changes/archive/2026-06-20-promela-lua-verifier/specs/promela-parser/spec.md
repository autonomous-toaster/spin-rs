## Task Reference

| Task ID | Description |
|---------|-------------|
| T1.1 | Lex Promela source into tokens |
| T1.2 | Parse tokens into AST |
| T1.3 | Report parse errors with source locations |

## ADDED Requirements

### Requirement: Tokenize Promela source

T1.1 SHALL complete BEFORE T1.2 SHALL parse tokens. T1.1 SHALL lex Promela source text into a token stream handling all Spin 6.5.x token types: keywords, operators, literals, identifiers, channel operations, labels, and preprocessor directives.

#### Scenario: Basic variable declarations

- **WHEN** T1.1 receives `byte x; bit flag; int counter = 0;`
- **THEN** T1.1 SHALL produce tokens for type keywords, identifiers, semicolons, and numeric literal `0`

#### Scenario: Channel declaration

- **WHEN** T1.1 receives `chan ch = [2] of { byte, int };`
- **THEN** T1.1 SHALL correctly tokenize `chan`, `ch`, `=`, `[`, `2`, `]`, `of`, `{`, `byte`, `,`, `int`, `}`, `;`

#### Scenario: Preprocessor directive passthrough

- **WHEN** T1.1 receives `#define N 5` or `#include "model.pml"`
- **THEN** T1.1 SHALL preserve preprocessor lines as raw tokens for the parser

### Requirement: Parse Promela into AST

T1.2 SHALL parse the token stream into a concrete syntax tree according to Promela grammar. T1.2 SHALL produce an AST suitable for semantic analysis and Lua code generation. T1.2 SHALL complete BEFORE T1.3 SHALL report errors.

#### Scenario: Proctype parsing

- **WHEN** T1.2 receives `active proctype P() { byte x; x = 1; }`
- **THEN** T1.2 SHALL produce an AST with a proctype node containing an assignment inside its body

#### Scenario: Control flow (if/fi, do/od)

- **WHEN** T1.2 receives `if :: (x > 0) -> y = 1 :: else -> y = 0 fi`
- **THEN** T1.2 SHALL parse the guarded if/fi with two options and an else branch

#### Scenario: Channel send/receive

- **WHEN** T1.2 receives `ch!msg(1)` and `ch?msg`
- **THEN** T1.2 SHALL parse synchronous channel send and receive expressions

### Requirement: Report parse errors

T1.3 SHALL report syntax errors with source file, line number, column, and descriptive message. T1.3 SHALL complete AFTER T1.2 SHALL detect the error.

#### Scenario: Missing semicolon

- **WHEN** T1.2 encounters `byte x byte y = 1` (missing semicolon)
- **THEN** T1.3 SHALL produce an error at the correct source line with context

#### Scenario: Unmatched keyword

- **WHEN** T1.2 encounters `if :: (x > 0) -> y = 1` (missing `fi`)
- **THEN** T1.3 SHALL report an unclosed `if` block
