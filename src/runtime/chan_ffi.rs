//! Channel FFI functions for the Lua runtime.
//!
//! **The channel queue lives in the state table, not in the runtime.** It used to live in a
//! registry shared by the whole runtime, keyed by channel name: `send` mutated the registry and
//! mirrored a copy into the state, while `full`/`len`/`empty` read only the registry. The state a
//! transition was expanded from therefore did not determine its own transitions - re-expanding an
//! earlier state saw the messages a later state had left behind. That produced a false liveness
//! refutation: after a send, the reopened initial state looked at a full channel, reported no
//! transitions, and the stutter step of a terminated system turned into an "accepting cycle".
//!
//! Only the channel's *capacity* is read from the registry. Capacity is static, so it is constant
//! across every state of a run, and the queue is not.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::LuaChannel;

type Registry = Arc<Mutex<HashMap<String, LuaChannel>>>;

/// Read a channel's queue out of the state table.
///
/// A single-value message is stored as a plain integer and a multi-value message as a table, so
/// the serialized state stays compact and old blobs keep their shape.
fn messages_of(state: &mlua::Table, name: &str) -> mlua::Result<Vec<Vec<i64>>> {
    let mut out = Vec::new();
    match state.get::<mlua::Value>(name)? {
        mlua::Value::Nil => {}
        mlua::Value::Table(t) => {
            let n = t.raw_len();
            for i in 1..=n {
                match t.get::<mlua::Value>(i as i64)? {
                    mlua::Value::Integer(v) => out.push(vec![v]),
                    mlua::Value::Number(v) => out.push(vec![v as i64]),
                    mlua::Value::Table(m) => {
                        let mut parts = Vec::new();
                        for j in 1..=m.raw_len() {
                            match m.get::<mlua::Value>(j as i64)? {
                                mlua::Value::Integer(v) => parts.push(v),
                                mlua::Value::Number(v) => parts.push(v as i64),
                                other => {
                                    return Err(mlua::Error::runtime(format!(
                                        "channel '{}': message component is {}",
                                        name,
                                        other.type_name()
                                    )));
                                }
                            }
                        }
                        out.push(parts);
                    }
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "channel '{}': message is {}",
                            name,
                            other.type_name()
                        )));
                    }
                }
            }
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "channel '{}': expected a table of messages, found {}",
                name,
                other.type_name()
            )));
        }
    }
    Ok(out)
}

/// Write a channel's queue back into the state table in the canonical shape.
fn write_messages(
    lua: &mlua::Lua,
    state: &mlua::Table,
    name: &str,
    msgs: &[Vec<i64>],
) -> mlua::Result<()> {
    let queue = lua.create_table()?;
    for (i, msg) in msgs.iter().enumerate() {
        if msg.len() == 1 {
            queue.set(i + 1, msg[0])?;
        } else {
            let inner = lua.create_table()?;
            for (j, v) in msg.iter().enumerate() {
                inner.set(j + 1, *v)?;
            }
            queue.set(i + 1, inner)?;
        }
    }
    state.set(name, queue)
}

/// A channel's capacity, which is static and so still comes from the declaration registry.
fn capacity_of(chans: &HashMap<String, LuaChannel>, name: &str) -> mlua::Result<usize> {
    chans.get(name).map(|c| c.capacity).ok_or_else(|| {
        mlua::Error::runtime(format!("channel '{}' not found in the declaration", name))
    })
}

/// Is the queue full, given its capacity? A rendezvous channel (capacity 0) is full when any
/// message is waiting, because a sender is blocked on it.
fn is_full(len: usize, capacity: usize) -> bool {
    capacity == 0 && len > 0 || (capacity > 0 && len >= capacity)
}

fn state_and_name(
    args: mlua::MultiValue,
    what: &str,
) -> mlua::Result<(mlua::Table, String, Vec<mlua::Value>)> {
    let mut iter = args.into_iter();
    let state = match iter.next() {
        Some(mlua::Value::Table(s)) => s,
        _ => {
            return Err(mlua::Error::runtime(format!(
                "{}: expected state table as first arg",
                what
            )));
        }
    };
    let name = match iter.next() {
        Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
        _ => {
            return Err(mlua::Error::runtime(format!(
                "{}: expected string channel name as second arg",
                what
            )));
        }
    };
    Ok((state, name, iter.collect()))
}

fn values_to_message(values: Vec<mlua::Value>, what: &str) -> mlua::Result<Vec<i64>> {
    let mut parts = Vec::new();
    for arg in values {
        match arg {
            mlua::Value::Integer(i) => parts.push(i),
            mlua::Value::Number(n) => parts.push(n as i64),
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "{}: expected integer values",
                    what
                )));
            }
        }
    }
    Ok(parts)
}

pub(crate) fn register_all(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
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
    register_chan_has_match(lua, Arc::clone(&channels))?;
    register_chan_recv_match(lua, Arc::clone(&channels))?;
    register_rendezvous(lua)?;
    Ok(())
}

/// A rendezvous channel, staged from the sender and released by the receiver.
///
/// A capacity-0 channel cannot be buffered: pan runs the send and the receive as **one**
/// transition, so the sender completes only when a receiver takes the message. This engine moves
/// one process per transition, so the pairing is expressed in two halves that the state carries:
///
/// - the sender *stages* `{values, owner, pid, cont}` into the channel and does not advance its
///   program counter, so its send step stays enabled-but-blocked (the guard rejects a second
///   stage);
/// - the receiver consumes the staged message, binds its variables, advances its own program
///   counter and writes `cont` into the stager's program counter, which is what completes the
///   send.
///
/// The stager's identity travels *in the message*, so the receiver does not have to know which
/// process will meet it at generation time.
fn register_rendezvous(lua: &mlua::Lua) -> mlua::Result<()> {
    let stage = lua.create_function(move |lua, args: mlua::MultiValue| -> mlua::Result<()> {
        let mut iter = args.into_iter();
        let state = match iter.next() {
            Some(mlua::Value::Table(t)) => t,
            _ => return Err(mlua::Error::runtime("stage: expected the state table")),
        };
        let name = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => return Err(mlua::Error::runtime("stage: expected a channel name")),
        };
        let owner = match iter.next() {
            Some(mlua::Value::String(s)) => s.to_string_lossy().to_string(),
            _ => return Err(mlua::Error::runtime("stage: expected the owner proctype")),
        };
        let pid = match iter.next() {
            Some(mlua::Value::Integer(i)) => i,
            _ => return Err(mlua::Error::runtime("stage: expected the pid")),
        };
        let cont = match iter.next() {
            Some(mlua::Value::Integer(i)) => i,
            _ => {
                return Err(mlua::Error::runtime(
                    "stage: expected the continuation step",
                ));
            }
        };
        let values = lua.create_table()?;
        for (i, v) in iter.enumerate() {
            match v {
                mlua::Value::Integer(i2) => values.set(i + 1, i2)?,
                mlua::Value::Number(n) => values.set(i + 1, n as i64)?,
                _ => return Err(mlua::Error::runtime("stage: expected integer values")),
            }
        }
        let staged = lua.create_table()?;
        staged.set("values", values)?;
        staged.set("owner", owner)?;
        staged.set("pid", pid)?;
        staged.set("cont", cont)?;
        state.set(name, staged)?;
        Ok(())
    })?;
    lua.globals().set("_spin_chan_stage", stage)?;

    let staged = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        Ok(matches!(
            state.get::<mlua::Value>(name.as_str())?,
            mlua::Value::Table(_)
        ))
    })?;
    lua.globals().set("_spin_chan_staged", staged)?;

    // The value a stager is offering, by position, so a random receive can match against a constant
    // before deciding to take the message.
    let staged_value = lua.create_function(
        move |_lua, (state, name, index): (mlua::Table, String, i64)| match state
            .get::<mlua::Value>(name.as_str())?
        {
            mlua::Value::Table(t) => match t.get::<mlua::Value>("values")? {
                mlua::Value::Table(values) => match values.get::<mlua::Value>(index)? {
                    mlua::Value::Integer(v) => Ok(mlua::Value::Integer(v)),
                    mlua::Value::Number(n) => Ok(mlua::Value::Integer(n as i64)),
                    _ => Ok(mlua::Value::Nil),
                },
                _ => Ok(mlua::Value::Nil),
            },
            _ => Ok(mlua::Value::Nil),
        },
    )?;
    lua.globals().set("_spin_chan_staged_value", staged_value)?;

    let pair = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        match state.get::<mlua::Value>(name.as_str())? {
            mlua::Value::Table(t) => {
                state.set(name.as_str(), mlua::Value::Nil)?;
                // The values are handed back as a table the caller reads, and the stager's
                // identity with it.
                Ok(t)
            }
            _ => Err(mlua::Error::runtime("rendezvous: nothing staged")),
        }
    })?;
    lua.globals().set("_spin_chan_pair", pair)?;
    Ok(())
}

fn register_chan_send(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let (state, name, values) = state_and_name(args, "chan_send")?;
        let msg = values_to_message(values, "chan_send")?;
        let mut queue = messages_of(&state, &name)?;
        let capacity = capacity_of(&channels.lock().unwrap(), &name)?;
        if capacity == 0 {
            // A rendezvous send pairs with a receive in another process, so it is one transition
            // over two processes. This engine moves one process per transition, so the model is
            // refused rather than given a verdict it has not earned.
            return Err(mlua::Error::runtime(
                "a rendezvous channel (capacity 0) needs a transition over two processes, which is not implemented",
            ));
        }
        if queue.len() >= capacity {
            return Err(mlua::Error::runtime("channel full"));
        }
        queue.push(msg);
        write_messages(lua, &state, &name, &queue)?;
        Ok(())
    })?;
    lua.globals().set("_spin_chan_send", f)?;
    Ok(())
}

fn register_chan_send_sorted(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let (state, name, values) = state_and_name(args, "chan_send_sorted")?;
        let msg = values_to_message(values, "chan_send_sorted")?;
        let mut queue = messages_of(&state, &name)?;
        let capacity = capacity_of(&channels.lock().unwrap(), &name)?;
        if capacity == 0 {
            return Err(mlua::Error::runtime(
                "a rendezvous channel (capacity 0) needs a transition over two processes, which is not implemented",
            ));
        }
        if queue.len() >= capacity {
            return Err(mlua::Error::runtime("channel full"));
        }
        let val = msg.first().copied().unwrap_or(0);
        let pos = queue
            .iter()
            .position(|m| m.first().copied().unwrap_or(0) > val)
            .unwrap_or(queue.len());
        queue.insert(pos, msg);
        write_messages(lua, &state, &name, &queue)?;
        Ok(())
    })?;
    lua.globals().set("_spin_chan_send_sorted", f)?;
    Ok(())
}

/// Shared body of every receive: take a message matching `select`, if any.
fn do_recv(
    lua: &mlua::Lua,
    args: mlua::MultiValue,
    what: &str,
    select: impl Fn(&[Vec<i64>]) -> Option<usize>,
) -> mlua::Result<mlua::Value> {
    let (state, name, _) = state_and_name(args, what)?;
    let mut queue = messages_of(&state, &name)?;
    match select(&queue) {
        Some(idx) => {
            let msg = queue.remove(idx);
            write_messages(lua, &state, &name, &queue)?;
            Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0)))
        }
        None => Err(mlua::Error::runtime("channel empty")),
    }
}

fn register_chan_recv(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        do_recv(lua, args, "chan_recv", |q| (!q.is_empty()).then_some(0))
    })?;
    lua.globals().set("_spin_chan_recv", f)?;
    Ok(())
}

fn register_chan_recv_random(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        do_recv(lua, args, "chan_recv_random", |q| {
            (!q.is_empty()).then_some(0)
        })
    })?;
    lua.globals().set("_spin_chan_recv_random", f)?;
    Ok(())
}

/// The index of the first message whose leading fields equal `expected`, if any.
///
/// spin's random receive scans the queue from the head and takes the *first* match, which is why
/// this is a search rather than a choice: measured on `c ! 1; c ! 2; c ! 1; c ?? 1`, pan removes
/// the message at the head and leaves [2, 1], not the later 1. A pattern of bare variables matches
/// every message, so for `c ?? x` the first match is the head - the same thing the plain receive
/// does.
fn match_index(queue: &[Vec<i64>], expected: &[i64]) -> Option<usize> {
    queue.iter().position(|msg| {
        msg.len() >= expected.len() && msg.iter().zip(expected).all(|(a, b)| a == b)
    })
}

/// Read the values to match, which are every argument after (state, name).
fn expected_values(args: &mlua::MultiValue, what: &str) -> mlua::Result<Vec<i64>> {
    let mut out = Vec::new();
    for value in args.clone().into_iter().skip(2) {
        match value {
            mlua::Value::Integer(i) => out.push(i),
            mlua::Value::Number(n) => out.push(n as i64),
            // Never guess at a non-numeric field: a silent skip would turn `c ?? 2` into
            // `c ?? <anything>` and decide a model against a pattern nobody wrote.
            other => {
                return Err(mlua::Error::runtime(format!(
                    "{}: a match value must be an integer, got {other:?}",
                    what
                )));
            }
        }
    }
    Ok(out)
}

/// Is there a message matching these values? The guard half of `_spin_chan_recv_match`.
fn register_chan_has_match(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, args: mlua::MultiValue| {
        let expected = expected_values(&args, "chan_has_match")?;
        let (state, name, _) = state_and_name(args, "chan_has_match")?;
        let queue = messages_of(&state, &name)?;
        Ok(match_index(&queue, &expected).is_some())
    })?;
    lua.globals().set("_spin_chan_has_match", f)?;
    Ok(())
}

/// Take the first message matching these values. The effect half.
fn register_chan_recv_match(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        let expected = expected_values(&args, "chan_recv_match")?;
        do_recv(lua, args, "chan_recv_match", |q| match_index(q, &expected))
    })?;
    lua.globals().set("_spin_chan_recv_match", f)?;
    Ok(())
}
fn register_chan_recv_eval(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |lua, args: mlua::MultiValue| {
        // The expected value is the third argument; it must be read before the shared body
        // consumes the argument list.
        let mut peek = args.clone().into_iter();
        let _state = peek.next();
        let _name = peek.next();
        let expected = match peek.next() {
            Some(mlua::Value::Integer(i)) => i,
            Some(mlua::Value::Number(n)) => n as i64,
            _ => {
                return Err(mlua::Error::runtime(
                    "chan_recv_eval: expected integer value",
                ));
            }
        };
        do_recv(lua, args, "chan_recv_eval", |q| {
            let head = q.first()?.first().copied().unwrap_or(0);
            (head == expected).then_some(0)
        })
    })?;
    lua.globals().set("_spin_chan_recv_eval", f)?;
    Ok(())
}

fn register_chan_peek(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        // An undeclared channel is an error rather than an empty one: answering "empty" would
        // turn a typo in a generated name into a model that silently never receives.
        capacity_of(&channels.lock().unwrap(), &name)?;
        let queue = messages_of(&state, &name)?;
        match queue.first() {
            Some(msg) => Ok(mlua::Value::Integer(msg.first().copied().unwrap_or(0))),
            None => Err(mlua::Error::runtime("channel empty")),
        }
    })?;
    lua.globals().set("_spin_chan_peek", f)?;
    Ok(())
}

fn register_chan_len(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        capacity_of(&channels.lock().unwrap(), &name)?;
        Ok(messages_of(&state, &name)?.len() as i64)
    })?;
    lua.globals().set("_spin_chan_len", f)?;
    Ok(())
}

fn register_chan_full(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        let len = messages_of(&state, &name)?.len();
        let capacity = capacity_of(&channels.lock().unwrap(), &name)?;
        Ok(is_full(len, capacity))
    })?;
    lua.globals().set("_spin_chan_full", f)?;
    Ok(())
}

fn register_chan_empty(lua: &mlua::Lua, channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(move |_lua, (state, name): (mlua::Table, String)| {
        capacity_of(&channels.lock().unwrap(), &name)?;
        Ok(messages_of(&state, &name)?.is_empty())
    })?;
    lua.globals().set("_spin_chan_empty", f)?;
    Ok(())
}

fn register_chan_poll(lua: &mlua::Lua, _channels: Registry) -> mlua::Result<()> {
    let f = lua.create_function(
        move |_lua, (state, name, expected): (mlua::Table, String, i64)| {
            let queue = messages_of(&state, &name)?;
            match queue.first().and_then(|m| m.first().copied()) {
                Some(v) if v == expected => Ok(mlua::Value::Integer(v)),
                _ => Ok(mlua::Value::Nil),
            }
        },
    )?;
    lua.globals().set("_spin_chan_poll", f)?;
    Ok(())
}
