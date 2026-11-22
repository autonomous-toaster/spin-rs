//! The small preprocessor Promela models rely on.
//!
//! Spin runs its own preprocessor before parsing, so macros are part of the language: `#define N 4`
//! followed by `byte q[N]` is ordinary Promela. Without it, 37 of the 78 models shipped in spin's
//! own `Examples/` do not parse at all.
//!
//! What is supported is object-like `#define` and nothing else. Function-like macros, `#include` and
//! conditionals are refused by name rather than approximated: expanding a function-like macro
//! wrongly produces a model that means something different, and a wrong verdict is worse than no
//! verdict. Models that use them are counted as refusals, which is where that debt belongs.

use std::collections::BTreeMap;

/// Why the source could not be preprocessed.
#[derive(Debug)]
pub struct Unsupported {
    pub directive: String,
    pub line: usize,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}: {} is not implemented (object-like #define is the only directive supported)",
            self.line, self.directive
        )
    }
}

/// Expand object-like macros and strip the directives.
///
/// Returns the source unchanged when it carries no directive, so a model that does not use the
/// preprocessor is never rewritten.
pub fn preprocess(source: &str) -> Result<String, Unsupported> {
    if !source.lines().any(|l| l.trim_start().starts_with('#')) {
        return Ok(source.to_string());
    }

    let mut macros: BTreeMap<String, String> = BTreeMap::new();
    let mut kept: Vec<&str> = Vec::new();

    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix('#') else {
            kept.push(line);
            continue;
        };
        let rest = rest.trim_start();
        let mut parts = rest.splitn(2, char::is_whitespace);
        let directive = parts.next().unwrap_or("").trim();
        let body = parts.next().unwrap_or("").trim();

        match directive {
            "define" => {
                // `#define NAME value` or `#define NAME(args) value`.
                let name_end = body
                    .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(body.len());
                let name = &body[..name_end];
                if name.is_empty() {
                    return Err(Unsupported {
                        directive: "#define without a name".to_string(),
                        line: i + 1,
                    });
                }
                if body[name_end..].trim_start().starts_with('(') {
                    return Err(Unsupported {
                        directive: format!("function-like macro '{}'", name),
                        line: i + 1,
                    });
                }
                // A trailing comment belongs to the comment, not to the value.
                let value = strip_comment(body[name_end..].trim());
                macros.insert(name.to_string(), value);
            }
            // A blank directive line, or one this file has already consumed.
            "" => {}
            "include" => {
                return Err(Unsupported {
                    directive: format!("#include {}", body),
                    line: i + 1,
                });
            }
            "if" | "ifdef" | "ifndef" | "else" | "elif" | "endif" => {
                return Err(Unsupported {
                    directive: format!("#{}", directive),
                    line: i + 1,
                });
            }
            _ => {
                return Err(Unsupported {
                    directive: format!("#{}", directive),
                    line: i + 1,
                });
            }
        }
    }

    let mut out = kept.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }

    // Expand to a fixed point. A macro that refers to another one is expanded on the next pass;
    // the bound keeps a self-referential define from looping forever.
    for _ in 0..8 {
        let before = out.clone();
        for (name, value) in &macros {
            out = replace_word(&out, name, value);
        }
        if out == before {
            break;
        }
    }
    Ok(out)
}

/// Is `word` present as an identifier outside comments and string literals?
///
/// Used to refuse a construct by name. A bare parse error names a character offset, which a caller
/// cannot tell apart from a malformed model of their own; naming the construct says what to do.
pub fn contains_keyword(source: &str, word: &str) -> bool {
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Skip comments.
        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // Skip string literals.
        if chars[i] == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            i = (i + 1).min(chars.len());
            continue;
        }
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            if chars[start..i].iter().collect::<String>() == word {
                return true;
            }
            continue;
        }
        i += 1;
    }
    false
}

/// Replace whole-word occurrences, leaving string literals and comments alone.
fn replace_word(text: &str, word: &str, value: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        // Copy comments and string literals verbatim: a macro name inside one is text.
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == '*' {
            while i < bytes.len()
                && !(bytes[i] == '*' && i + 1 < bytes.len() && bytes[i + 1] == '/')
            {
                out.push(bytes[i]);
                i += 1;
            }
            if i + 1 < bytes.len() {
                out.push(bytes[i]);
                out.push(bytes[i + 1]);
                i += 2;
            }
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == '/' {
            while i < bytes.len() && bytes[i] != '\n' {
                out.push(bytes[i]);
                i += 1;
            }
            continue;
        }
        if c == '"' {
            out.push(c);
            i += 1;
            while i < bytes.len() {
                out.push(bytes[i]);
                if bytes[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            let ident: String = bytes[start..i].iter().collect();
            if ident == word {
                out.push_str(value);
            } else {
                out.push_str(&ident);
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Drop a trailing `/* ... */` or `// ...` from a macro value.
fn strip_comment(s: &str) -> String {
    let cut = s
        .find("/*")
        .into_iter()
        .chain(s.find("//"))
        .min()
        .unwrap_or(s.len());
    s[..cut].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_like_define_is_expanded() {
        let src = "#define N 4\nbyte q[N];\n";
        assert_eq!(preprocess(src).unwrap(), "byte q[4];\n");
    }

    #[test]
    fn a_macro_inside_a_comment_or_string_is_text() {
        let src = "#define N 4\n/* N is the size */\nprintf(\"N\");\nbyte q[N];\n";
        let out = preprocess(src).unwrap();
        assert!(out.contains("/* N is the size */"), "{out}");
        assert!(out.contains("printf(\"N\")"), "{out}");
        assert!(out.contains("byte q[4]"), "{out}");
    }

    #[test]
    fn a_macro_that_names_another_is_expanded() {
        let src = "#define A 2\n#define B A\nbyte q[B];\n";
        assert_eq!(preprocess(src).unwrap(), "byte q[2];\n");
    }

    #[test]
    fn a_function_like_macro_is_refused_by_name() {
        let err = preprocess("#define Dist(a,b) distance[a]\n").unwrap_err();
        assert!(
            err.to_string().contains("function-like macro 'Dist'"),
            "{err}"
        );
    }

    #[test]
    fn an_include_is_refused() {
        let err = preprocess("#include \"x.h\"\n").unwrap_err();
        assert!(err.to_string().contains("#include"), "{err}");
    }

    #[test]
    fn a_source_with_no_directive_is_untouched() {
        let src = "byte x;\nactive proctype P() { x = 1 }\n";
        assert_eq!(preprocess(src).unwrap(), src);
    }

    #[test]
    fn a_prefix_of_an_identifier_is_not_replaced() {
        let src = "#define N 4\nbyte NN;\nbyte q[N];\n";
        let out = preprocess(src).unwrap();
        assert!(out.contains("byte NN;"), "{out}");
        assert!(out.contains("byte q[4];"), "{out}");
    }
}
