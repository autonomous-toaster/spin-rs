// ─── Serialization ──────────────────────────────────────────────

// Reused across calls: the state table's shape does not change during a search, so the entry
// buffer is allocated once rather than per serialization. Serialization runs once per enabled
// transition per state, which makes this the single hottest allocation in the engine.
thread_local! {
    static ENTRIES: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

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
    // Hand the buffer back for the next call, so the allocation is reused rather than remade on
    // every serialization.
    ENTRIES.with(|e| {
        let mut slot = e.borrow_mut();
        if slot.capacity() < entries.capacity() {
            *slot = entries;
        }
    });
    Ok(out)
}

/// Build a Lua literal expression to reconstruct a serialized state table.
/// The entry's key, with any quoting removed.
fn entry_key(entry: &str) -> &str {
    let mut depth = 0;
    for (i, ch) in entry.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ':' if depth == 0 => {
                return entry[..i].trim().trim_matches('"');
            }
            _ => {}
        }
    }
    entry.trim().trim_matches('"')
}

/// Project a serialized state onto a set of fields.
///
/// Two states that differ only in fields no claim can read are the same state for that claim.
/// pan's dead-variable analysis stores them once; without this a generated model doubles per such
/// bit. The projection is applied before hashing and before storing, so the key and the value are
/// projected the same way.
///
/// Keys are kept in the blob's own order, which `serialize_table` sorts, so the result is
/// canonical: two equal projections produce byte-identical strings and therefore equal hashes.
///
/// Returning the input unchanged when `visible` is empty is deliberate. An empty set means the
/// caller does not know what the claim observes, and a projection that drops fields it does not
/// understand would hide findings rather than speed up a search.
pub(crate) fn project_blob(blob: &str, visible: &std::collections::BTreeSet<String>) -> String {
    if visible.is_empty() {
        return blob.to_string();
    }
    // Built in one pass over slices. The previous form allocated a String per field (char by
    // char), collected them, then copied each kept one again - three allocations per field, on a
    // function that runs once per state visit. On a 180-field state that was the difference
    // between 0.89 ms and a fraction of it per state.
    let inner = blob
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(blob);
    let mut out = String::with_capacity(blob.len());
    out.push('{');
    let mut first = true;
    let mut depth = 0usize;
    let mut start = 0usize;
    let bytes = inner.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let at_end = i + 1 == bytes.len();
        if (b == b',' && depth == 0) || at_end {
            let take = if at_end { i + 1 } else { i };
            let entry = inner[start..take].trim();
            start = take + 1;
            if entry.is_empty() {
                continue;
            }
            if !visible.contains(entry_key(entry)) {
                continue;
            }
            if !first {
                out.push(',');
            }
            out.push_str(entry);
            first = false;
        }
    }
    out.push('}');
    out
}

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
