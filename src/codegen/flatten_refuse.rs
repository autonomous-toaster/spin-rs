//! The shapes this engine declines, decided from the statement alone.
//!
//! Split from the layout so that file stays a layout. Every entry here is a shape pan either
//! rejects at run time or answers for a reason this engine cannot reproduce, so the only honest
//! outcome is a named refusal: no verdict is claimed, and the caller can act on the name.

use super::flatten::FNode;
use super::*;

impl LuaGenerator {
    /// Does a send or receive on this channel carry a different number of values than the
    /// channel's message has fields?
    ///
    /// pan rejects that at run time, so the model has no verdict - pan reports an error either
    /// way. Binding the first field and ignoring the rest is a silent wrong: on a two-field
    /// channel, `c ? a` decided 0 where pan reports 1.
    pub(crate) fn arity_refusal(
        &self,
        channel: &Expression,
        values: usize,
    ) -> Option<&'static str> {
        let fields = *self.channel_fields.get(&self.channel_name(channel))?;
        (fields != values).then_some(
            // No apostrophe: this text is spliced into a Lua string literal, and one would end it.
            "a send or receive whose value count differs from the declared field count of its channel is not supported",
        )
    }

    /// The node a statement gets when its shape alone settles that it has no encoding.
    pub(crate) fn arity_refuse(&self, s: &Stmt) -> Option<FNode> {
        let (channel, count) = match s {
            Stmt::Send { channel, args, .. } => (channel, 1 + args.len()),
            Stmt::Recv {
                channel, target, ..
            } => (
                channel,
                match target {
                    RecvTarget::VarList(v) => v.len(),
                    RecvTarget::Random(v) => v.len(),
                    RecvTarget::RandomMatch(e) => e.len(),
                    // An inspect-only receive compares against the head; whether pan requires it
                    // to name every field is unmeasured, so it is left alone rather than guessed.
                    RecvTarget::Poll(_) | RecvTarget::Eval(_) => return None,
                },
            ),
            _ => return None,
        };
        self.arity_refusal(channel, count).map(FNode::Refuse)
    }

    /// Emit a step that refuses by name.
    pub(crate) fn emit_refuse_step(&mut self, i: usize, why: &str) {
        self.emit(&format!("    -- step {}: refused, {}", i, why));
        // The text is a whole sentence. Appending "is not supported inside a body" turned a named
        // refusal into "a random receive of a value is not implemented is not supported inside a
        // body", which reads as two rules stacked.
        self.emit(&format!("    guard = function(s) error('{}') end,", why));
        self.emit("    effect = function(s) end,");
        self.emit_opaque_deps();
    }
}
