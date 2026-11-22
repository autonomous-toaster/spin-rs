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
