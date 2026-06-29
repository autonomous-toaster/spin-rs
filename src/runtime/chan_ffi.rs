//! Channel FFI functions for the Lua runtime.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::LuaChannel;

pub(crate) fn register_all(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    register_chan_send(lua, Arc::clone(&channels))?;
    register_chan_recv(lua, Arc::clone(&channels))?;
    register_chan_peek(lua, Arc::clone(&channels))?;
    register_chan_len(lua, Arc::clone(&channels))?;
    register_chan_full(lua, Arc::clone(&channels))?;
    register_chan_empty(lua, Arc::clone(&channels))?;
    register_chan_send_sorted(lua, Arc::clone(&channels))?;
    register_chan_recv_random(lua, Arc::clone(&channels))?;
    register_chan_poll(lua, Arc::clone(&channels))?;
    register_chan_recv_eval(lua, Arc::clone(&channels))?;
    Ok(())
}

fn register_chan_send(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut iter = args.into_iter();
        let state: mlua::Table = match iter.next() {
            Some(mlua::Value::Table(s)) => s,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_send: expected state table as first arg",
                ));
            }
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_send: expected string channel name as second arg",
                ));
            }
        };
        let mut parts = Vec::new();
        for arg in iter {
            match arg {
                mlua::Value::Integer(i) => parts.push(i),
                mlua::Value::Number(n) => parts.push(n as i64),
                _ => return Err(mlua::Error::runtime("chan_send: expected integer values")),
            }
        }
        let mut chans = channels.lock().unwrap();
        if let Some(chan) = chans.get_mut(&name) {
            if !chan.send(parts.clone()) {
                return Err(mlua::Error::runtime("channel full"));
            }
            let chan_state: mlua::Table = lua.create_table()?;
            for (i, val) in chan.messages.iter().enumerate() {
                if let Some(&first) = val.first() {
                    chan_state.set(i + 1, first)?;
                }
            }
            state.set(name.clone(), chan_state)?;
            Ok(())
        } else {
            Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            )))
        }
    })?;
    lua.globals().set("_spin_chan_send", f)?;
    Ok(())
}

fn register_chan_recv(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut iter = args.into_iter();
        let state: mlua::Table = match iter.next() {
            Some(mlua::Value::Table(s)) => s,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv: expected state table as first arg",
                ));
            }
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv: expected string channel name as second arg",
                ));
            }
        };
        let mut chans = channels.lock().unwrap();
        if let Some(chan) = chans.get_mut(&name) {
            match chan.recv() {
                Some(msg) => {
                    let chan_state: mlua::Table = lua.create_table()?;
                    for (i, val) in chan.messages.iter().enumerate() {
                        if let Some(&first) = val.first() {
                            chan_state.set(i + 1, first)?;
                        }
                    }
                    state.set(name.clone(), chan_state)?;
                    Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0)))
                }
                None => Err(mlua::Error::runtime("channel empty")),
            }
        } else {
            Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            )))
        }
    })?;
    lua.globals().set("_spin_chan_recv", f)?;
    Ok(())
}

fn register_chan_peek(
    lua: &mlua::Lua,
    _channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        let chan_state: mlua::Table = match state.get(name.as_str()) {
            Ok(t) => t,
            Err(_) => return Err(mlua::Error::runtime("channel not found in state")),
        };
        match chan_state.get(1i32) {
            Ok(mlua::Value::Integer(i)) => Ok(mlua::Value::Integer(i)),
            Ok(mlua::Value::Nil) => Err(mlua::Error::runtime("channel empty")),
            Err(e) => Err(mlua::Error::runtime(format!("channel peek error: {}", e))),
            _ => Err(mlua::Error::runtime("channel peek: unexpected value type")),
        }
    })?;
    lua.globals().set("_spin_chan_peek", f)?;
    Ok(())
}

fn register_chan_len(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, name: String| {
        let chans = channels.lock().unwrap();
        match chans.get(&name) {
            Some(chan) => Ok(chan.len() as i64),
            None => Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            ))),
        }
    })?;
    lua.globals().set("_spin_chan_len", f)?;
    Ok(())
}

fn register_chan_full(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, name: String| {
        let chans = channels.lock().unwrap();
        match chans.get(&name) {
            Some(chan) => Ok(chan.is_full()),
            None => Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            ))),
        }
    })?;
    lua.globals().set("_spin_chan_full", f)?;
    Ok(())
}

fn register_chan_empty(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, name: String| {
        let chans = channels.lock().unwrap();
        match chans.get(&name) {
            Some(chan) => Ok(chan.is_empty()),
            None => Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            ))),
        }
    })?;
    lua.globals().set("_spin_chan_empty", f)?;
    Ok(())
}

fn register_chan_send_sorted(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut iter = args.into_iter();
        let state: mlua::Table = match iter.next() {
            Some(mlua::Value::Table(s)) => s,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_send_sorted: expected state table as first arg",
                ));
            }
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_send_sorted: expected string channel name",
                ));
            }
        };
        let mut parts = Vec::new();
        for arg in iter {
            match arg {
                mlua::Value::Integer(i) => parts.push(i),
                mlua::Value::Number(n) => parts.push(n as i64),
                _ => {
                    return Err(mlua::Error::runtime(
                        "chan_send_sorted: expected integer values",
                    ));
                }
            }
        }
        let mut chans = channels.lock().unwrap();
        if let Some(chan) = chans.get_mut(&name) {
            if !chan.send_sorted(parts.clone()) {
                return Err(mlua::Error::runtime("channel full"));
            }
            let chan_state: mlua::Table = lua.create_table()?;
            for (i, val) in chan.messages.iter().enumerate() {
                if let Some(&first) = val.first() {
                    chan_state.set(i + 1, first)?;
                }
            }
            state.set(name.clone(), chan_state)?;
            Ok(())
        } else {
            Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            )))
        }
    })?;
    lua.globals().set("_spin_chan_send_sorted", f)?;
    Ok(())
}

fn register_chan_recv_random(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut iter = args.into_iter();
        let state: mlua::Table = match iter.next() {
            Some(mlua::Value::Table(s)) => s,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_random: expected state table as first arg",
                ));
            }
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_random: expected string channel name",
                ));
            }
        };
        let mut chans = channels.lock().unwrap();
        if let Some(chan) = chans.get_mut(&name) {
            match chan.recv_random() {
                Some(msg) => {
                    let chan_state: mlua::Table = lua.create_table()?;
                    for (i, val) in chan.messages.iter().enumerate() {
                        if let Some(&first) = val.first() {
                            chan_state.set(i + 1, first)?;
                        }
                    }
                    state.set(name.clone(), chan_state)?;
                    Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0)))
                }
                None => Err(mlua::Error::runtime("channel empty")),
            }
        } else {
            Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            )))
        }
    })?;
    lua.globals().set("_spin_chan_recv_random", f)?;
    Ok(())
}

fn register_chan_poll(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (name, expected): (String, i64)| {
        let chans = channels.lock().unwrap();
        match chans.get(&name) {
            Some(chan) => Ok(chan.poll(expected)),
            None => Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            ))),
        }
    })?;
    lua.globals().set("_spin_chan_poll", f)?;
    Ok(())
}

fn register_chan_recv_eval(
    lua: &mlua::Lua,
    channels: Arc<Mutex<HashMap<String, LuaChannel>>>,
) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let mut iter = args.into_iter();
        let state: mlua::Table = match iter.next() {
            Some(mlua::Value::Table(s)) => s,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_eval: expected state table as first arg",
                ));
            }
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_eval: expected string channel name",
                ));
            }
        };
        let expected = match iter.next() {
            Some(mlua::Value::Integer(i)) => i,
            Some(mlua::Value::Number(n)) => n as i64,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_eval: expected integer value",
                ));
            }
        };
        let mut chans = channels.lock().unwrap();
        if let Some(chan) = chans.get_mut(&name) {
            match chan.recv_eval(expected) {
                Some(msg) => {
                    let chan_state: mlua::Table = lua.create_table()?;
                    for (i, val) in chan.messages.iter().enumerate() {
                        if let Some(&first) = val.first() {
                            chan_state.set(i + 1, first)?;
                        }
                    }
                    state.set(name.clone(), chan_state)?;
                    Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0)))
                }
                None => Err(mlua::Error::runtime(
                    "channel eval receive: value mismatch or empty",
                )),
            }
        } else {
            Err(mlua::Error::runtime(format!(
                "channel '{}' not found",
                name
            )))
        }
    })?;
    lua.globals().set("_spin_chan_recv_eval", f)?;
    Ok(())
}
