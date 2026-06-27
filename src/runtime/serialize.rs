// ─── Serialization ──────────────────────────────────────────────

/// Serialize a Lua table to a deterministic string for hashing and equality.
pub(crate) fn serialize_table(table: &mlua::Table) -> mlua::Result<String> {
    let mut entries: Vec<(String, String)> = Vec::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        entries.push((val_to_string(&key), val_to_string(&value)));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::from("{");
    for (i, (k, v)) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(k);
        out.push(':');
        out.push_str(v);
    }
    out.push('}');
    Ok(out)
}

/// Build a Lua literal expression to reconstruct a serialized state table.
pub(crate) fn state_literal(blob: &str) -> String {
    // Remove outer braces
    let inner = if blob.starts_with('{') && blob.ends_with('}') {
        &blob[1..blob.len() - 1]
    } else {
        blob
    };
    if inner.is_empty() {
        return String::new();
    }

    // Parse entries respecting nested braces
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0;

    for ch in inner.chars() {
        match ch {
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                current.push(ch);
                brace_depth -= 1;
            }
            ',' => {
                if brace_depth == 0 {
                    entries.push(current.trim().to_string());
                    current = String::new();
                } else {
                    current.push(ch);
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        entries.push(current.trim().to_string());
    }

    entries
        .into_iter()
        .filter(|s| !s.is_empty())
        .filter_map(|entry| {
            // Find the first colon at depth 0 (not inside nested tables)
            let mut depth = 0;
            let mut colon_idx = None;
            for (i, ch) in entry.chars().enumerate() {
                match ch {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    ':' if depth == 0 => {
                        colon_idx = Some(i);
                        break;
                    }
                    _ => {}
                }
            }
            let idx = colon_idx?;
            let (k, v) = entry.split_at(idx);
            let v = &v[1..];
            Some(format!("s[{}] = {}", k, v))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Format a Lua value as a compact string for state serialization.
pub(crate) fn val_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => {
            if *n == (*n as i64) as f64 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        mlua::Value::String(s) => {
            if let Ok(s) = s.to_str() {
                format!("\"{}\"", s)
            } else {
                "nil".to_string()
            }
        }
        mlua::Value::Table(t) => {
            let mut parts: Vec<String> = Vec::new();
            for (k, v) in t.clone().pairs::<mlua::Value, mlua::Value>().flatten() {
                // Use Lua table constructor syntax: {[key]=value} for numeric keys
                let key_str = val_to_string(&k);
                let val_str = val_to_string(&v);
                parts.push(format!("[{}]={}", key_str, val_str));
            }
            parts.sort();
            format!("{{{}}}", parts.join(","))
        }
        _ => "nil".to_string(),
    }
}
