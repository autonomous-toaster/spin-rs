use super::*;
use crate::runtime::serialize::val_to_string;

#[test]
fn test_lua_runtime_init() {
    let rt = LuaRuntime::new().unwrap();
    // No init_state call since _spin_init_state requires generated code
    assert!(rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_printf")
        .is_ok());
    assert!(rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_assert")
        .is_ok());
    assert!(rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_send")
        .is_ok());
    assert!(rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_recv")
        .is_ok());
}

#[test]
fn test_channel_send_recv() {
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("q", 10);
    let chans = rt.channels.lock().unwrap();
    let chan = chans.get("q").unwrap();
    let mut c = chan.clone();
    assert!(c.send(vec![42]));
    assert_eq!(c.recv(), Some(vec![42]));
    assert!(c.is_empty());
}

#[test]
fn test_verify_simple() {
    let source = "active proctype P() { byte x; x = 1; }";
    let result = verify(source).unwrap();
    assert_eq!(result.errors, 0);
    assert!(result.states_explored > 0);
}

#[test]
fn test_val_to_string_nil() {
    assert_eq!(val_to_string(&mlua::Value::Nil), "nil");
}

#[test]
fn test_val_to_string_bool() {
    assert_eq!(val_to_string(&mlua::Value::Boolean(true)), "true");
    assert_eq!(val_to_string(&mlua::Value::Boolean(false)), "false");
}

#[test]
fn test_val_to_string_integer() {
    assert_eq!(val_to_string(&mlua::Value::Integer(42)), "42");
    assert_eq!(val_to_string(&mlua::Value::Integer(-1)), "-1");
}

#[test]
fn test_val_to_string_number() {
    assert_eq!(
        val_to_string(&mlua::Value::Number(std::f64::consts::PI)),
        "3.141592653589793"
    );
    assert_eq!(val_to_string(&mlua::Value::Number(5.0)), "5");
}

#[test]
fn test_channel_full_empty() {
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("q", 1);
    {
        let chans = rt.channels.lock().unwrap();
        let chan = chans.get("q").unwrap();
        assert!(chan.is_empty());
        assert!(!chan.is_full());
        assert_eq!(chan.len(), 0);
    }
    // Send one message to fill channel
    rt.register_channel("q", 1);
    {
        let mut chans = rt.channels.lock().unwrap();
        let chan = chans.get_mut("q").unwrap();
        assert!(chan.send(vec![1]));
        assert!(chan.is_full());
        assert!(!chan.is_empty());
        assert_eq!(chan.len(), 1);
    }
}

#[test]
fn test_channel_send_full() {
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("q", 1);
    {
        let mut chans = rt.channels.lock().unwrap();
        let chan = chans.get_mut("q").unwrap();
        assert!(chan.send(vec![1]));
        // Channel has capacity 1, second send should fail
        assert!(!chan.send(vec![2]));
    }
}

#[test]
fn test_channel_recv_empty() {
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("q", 1);
    {
        let mut chans = rt.channels.lock().unwrap();
        let chan = chans.get_mut("q").unwrap();
        assert_eq!(chan.recv(), None);
    }
}

#[test]
fn test_channel_unbuffered() {
    let mut c = LuaChannel::new(0);
    // Capacity 0 means unbounded in current implementation
    assert!(c.send(vec![1]));
}

#[test]
fn test_register_functions_via_lua() {
    // Test that channel functions work properly through Lua API
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("test", 5);

    // Create a state table for channel operations
    let state: mlua::Table = rt.lua.create_table().unwrap();

    // Test chan_send via Lua (now requires state table as first arg)
    let send_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_send")
        .unwrap();
    let result = send_fn.call::<()>((state.clone(), "test", 42i64));
    assert!(result.is_ok());

    // Test chan_len via Lua (still uses Rust registry, not state table)
    let len_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_len")
        .unwrap();
    let len: i64 = len_fn.call("test").unwrap();
    assert_eq!(len, 1);

    // Test chan_recv via Lua (now requires state table as first arg)
    let recv_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_recv")
        .unwrap();
    let val: i64 = recv_fn.call((state.clone(), "test")).unwrap();
    assert_eq!(val, 42);

    // Test chan_empty via Lua
    let empty_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_empty")
        .unwrap();
    let empty: bool = empty_fn.call("test").unwrap();
    assert!(empty);

    // Test chan_full via Lua
    let full_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_full")
        .unwrap();
    let full: bool = full_fn.call("test").unwrap();
    assert!(!full);
}

#[test]
fn test_register_functions_error_paths() {
    let mut rt = LuaRuntime::new().unwrap();
    rt.register_channel("exists", 5);

    // Test send to non-existent channel
    let send_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_send")
        .unwrap();
    let result = send_fn.call::<()>(("nonexistent", 1i64));
    assert!(result.is_err());

    // Test recv from non-existent channel
    let recv_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_recv")
        .unwrap();
    let result = recv_fn.call::<i64>(("nonexistent",));
    assert!(result.is_err());

    // Test len on non-existent channel
    let len_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_len")
        .unwrap();
    let result = len_fn.call::<i64>("nonexistent");
    assert!(result.is_err());

    // Test full on non-existent channel
    let full_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_full")
        .unwrap();
    let result = full_fn.call::<bool>("nonexistent");
    assert!(result.is_err());

    // Test empty on non-existent channel
    let empty_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_chan_empty")
        .unwrap();
    let result = empty_fn.call::<bool>("nonexistent");
    assert!(result.is_err());

    // Test send with wrong argument type
    let result = send_fn.call::<()>((42i64, 1i64));
    assert!(result.is_err());

    // Test recv with wrong argument type
    let result = recv_fn.call::<i64>((42i64,));
    assert!(result.is_err());

    // Test chan_send with non-integer value
    let result = send_fn.call::<()>(("exists", "bad"));
    assert!(result.is_err());

    // Test printf
    let printf_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_printf")
        .unwrap();
    let result = printf_fn.call::<()>(("test", "arg"));
    assert!(result.is_ok());
}

#[test]
fn test_register_functions_assert() {
    let rt = LuaRuntime::new().unwrap();
    let assert_fn = rt
        .lua
        .globals()
        .get::<mlua::Function>("_spin_assert")
        .unwrap();

    // Assert true should pass
    let result = assert_fn.call::<()>((true, "should pass".to_string()));
    assert!(result.is_ok());

    // Assert false should fail
    let result = assert_fn.call::<()>((false, "should fail".to_string()));
    assert!(result.is_err());
}

#[test]
fn test_channel_array_registration() {
    // Test that chan tok[5] properly registers 5 channels
    let source = "chan tok[5]; active proctype P() { byte x; x = 1; }";
    let model = crate::parser::parse(source).unwrap();
    let generated = crate::codegen::generate(&model);
    let mut rt = LuaRuntime::new().unwrap();
    rt.load_source(&generated).unwrap();

    // Register channels from model (this is what from_model does)
    for decl in &model.declarations {
        if let crate::parser::ast::TopLevel::ChannelArray { name, size, .. } = decl {
            for i in 0..*size {
                let chan_name = format!("{}_{}", name, i);
                rt.register_channel(&chan_name, 0);
            }
        }
    }

    // Verify all 5 channels are registered
    for i in 0..5 {
        let chan_name = format!("tok_{}", i);
        assert!(
            rt.channels.lock().unwrap().contains_key(&chan_name),
            "Channel '{}' should be registered",
            chan_name
        );
    }
}
