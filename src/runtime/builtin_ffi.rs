//! Built-in function FFI for the Lua runtime.

pub(crate) fn register_all(lua: &mlua::Lua) -> mlua::Result<()> {
    register_assert(lua)?;
    register_printf(lua)?;
    register_state_hash(lua)?;
    register_remote_ref(lua)?;
    register_fairness_track(lua)?;
    register_c_code(lua)?;
    register_stubborn_dep(lua)?;
    register_enabled(lua)?;
    register_timeout(lua)?;
    register_np_(lua)?;
    register_pc_value(lua)?;
    register_truthy(lua)?;
    register_wrap(lua)?;
    register_int_ops(lua)?;
    register_get_priority(lua)?;
    register_set_priority(lua)?;
    Ok(())
}

fn register_assert(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (cond, msg): (bool, String)| {
        if !cond {
            Err(mlua::Error::runtime(msg))
        } else {
            Ok(())
        }
    })?;
    lua.globals().set("_spin_assert", f)?;
    Ok(())
}

fn register_printf(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, args: mlua::MultiValue| {
        let parts: Vec<String> = args.iter().map(|v| format!("{:?}", v)).collect();
        eprintln!("{}", parts.join(" "));
        Ok(())
    })?;
    lua.globals().set("_spin_printf", f)?;
    Ok(())
}

fn register_state_hash(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, state: mlua::Table| {
        crate::runtime::serialize::serialize_table(&state)
    })?;
    lua.globals().set("_spin_state_hash", f)?;
    Ok(())
}

fn register_remote_ref(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (pid, var): (i64, String)| {
        Ok(format!("<remote {}:{}>", pid, var))
    })?;
    lua.globals().set("_spin_remote_ref", f)?;
    Ok(())
}

fn register_fairness_track(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (_label, _enabled): (String, bool)| Ok(()))?;
    lua.globals().set("_spin_fairness_track", f)?;
    Ok(())
}

fn register_c_code(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|lua, code: String| {
        lua.load(&code)
            .exec()
            .map_err(|e| mlua::Error::runtime(format!("c_code: {}", e)))
    })?;
    lua.globals().set("_spin_c_code", f)?;
    Ok(())
}

fn register_stubborn_dep(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (_t1, _t2): (String, String)| Ok(false))?;
    lua.globals().set("_spin_stubborn_dep", f)?;
    Ok(())
}

fn register_enabled(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (state, pid): (mlua::Table, i64)| {
        let done_key = format!("_done_{}", pid);
        match state.get::<bool>(done_key.as_str()) {
            Ok(done) => Ok(!done),
            Err(_) => Ok(false),
        }
    })?;
    lua.globals().set("_spin_enabled", f)?;
    Ok(())
}

fn register_timeout(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, ()| Ok(false))?;
    lua.globals().set("_spin_timeout", f)?;
    Ok(())
}

fn register_np_(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (state,): (mlua::Table,)| {
        for pair in state.pairs::<mlua::Value, mlua::Value>() {
            if let Ok((key, _)) = pair
                && let mlua::Value::String(s) = key
            {
                let key_str = s.to_string_lossy();
                if key_str.starts_with("_progress_")
                    && let Ok(val) = state.get::<bool>(key_str.as_str())
                    && val
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    })?;
    lua.globals().set("_spin_np_", f)?;
    Ok(())
}

/// Integer division and remainder with C's semantics, which are Promela's.
///
/// Lua's `/` is floating point and its `%` floors: `-7 % 2` is 1 in Lua and -1 in C, so a claim of
/// the form `(x % 2 == -1)` was false here and true in pan. Division truncates toward zero, and
/// dividing by zero is refused rather than producing an infinity that then gets truncated.
fn register_int_ops(lua: &mlua::Lua) -> mlua::Result<()> {
    let div = lua.create_function(|_lua, (a, b): (i64, i64)| {
        if b == 0 {
            return Err(mlua::Error::runtime("division by zero"));
        }
        Ok(a / b)
    })?;
    lua.globals().set("_spin_div", div)?;
    let rem = lua.create_function(|_lua, (a, b): (i64, i64)| {
        if b == 0 {
            return Err(mlua::Error::runtime("modulo by zero"));
        }
        Ok(a % b)
    })?;
    lua.globals().set("_spin_mod", rem)?;
    Ok(())
}

/// Truncate a value to a Promela type.
///
/// Promela's types are fixed width and assignments wrap: `byte x = 255; x = x + 1` leaves x at 0, and
/// `bit b = 2` leaves b at 0. Lua numbers are arbitrary precision, so every stored value went out of
/// range and a claim that pan satisfies was refuted - or, in the four probe models that found this,
/// a claim that pan refutes was reported clean.
///
/// The argument is the type's width in bits and whether it is signed; the state stores the value
/// after truncation, exactly as pan's state vector does.
fn register_wrap(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (width, signed, v): (i64, bool, i64)| {
        if width <= 0 {
            return Ok(v);
        }
        let modulus = 1i64.checked_shl(width as u32).unwrap_or(0);
        if modulus == 0 {
            return Ok(v);
        }
        let mut r = v % modulus;
        if r < 0 {
            r += modulus;
        }
        if signed && r >= modulus / 2 {
            r -= modulus;
        }
        Ok(r)
    })?;
    lua.globals().set("_spin_wrap", f)?;
    Ok(())
}

/// Promela truthiness.
///
/// A guard is true when its value is non-zero and not `false`/`nil`. Lua's only falsey values are
/// `false` and `nil`, so `if :: (n) ->` with `n == 0` was enabled in the generated code while pan
/// treats it as disabled - a false refutation of any model whose claim observes what that branch
/// does. Every guard, condition and blocking expression goes through here.
fn register_truthy(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, v: mlua::Value| match v {
        mlua::Value::Nil => Ok(false),
        mlua::Value::Boolean(b) => Ok(b),
        mlua::Value::Integer(i) => Ok(i != 0),
        mlua::Value::Number(n) => Ok(n != 0.0),
        mlua::Value::String(s) => Ok(!s.to_string_lossy().is_empty()),
        // A table or function has no Promela meaning; treat it as true rather than silently
        // disabling a transition.
        _ => Ok(true),
    })?;
    lua.globals().set("_spin_truthy", f)?;
    Ok(())
}

/// `pc_value(pid)` refuses.
///
/// It used to read `_pc_<pid>`, a field that is never written: the generator writes
/// `_pc_<proctype>`, so every call answered a confident 0 and a model that branched on it was
/// decided against a number nobody had computed. Refusing is the honest answer, and it will stay
/// the honest answer even once the field is right, because pan reports its own statement
/// numbering and no model can expect the two to be equal.
fn register_pc_value(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, _: mlua::MultiValue| -> mlua::Result<i64> {
        Err(mlua::Error::runtime(
            "pc_value is not supported: it exposes the checker's own statement numbering, which is not comparable with pan's",
        ))
    })?;
    lua.globals().set("_spin_pc_value_unsupported", f)?;
    Ok(())
}

fn register_get_priority(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (state, pid): (mlua::Table, i64)| {
        let prio_key = format!("_priority_{}", pid);
        match state.get::<i64>(prio_key.as_str()) {
            Ok(prio) => Ok(prio),
            Err(_) => Ok(0),
        }
    })?;
    lua.globals().set("_spin_get_priority", f)?;
    Ok(())
}

fn register_set_priority(lua: &mlua::Lua) -> mlua::Result<()> {
    let f = lua.create_function(|_lua, (state, pid, val): (mlua::Table, i64, i64)| {
        let prio_key = format!("_priority_{}", pid);
        state.set(prio_key.as_str(), val)?;
        Ok(())
    })?;
    lua.globals().set("_spin_set_priority", f)?;
    Ok(())
}
