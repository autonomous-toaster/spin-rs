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

    /// The node a send or receive gets when its shape alone settles that it has no encoding.
    ///
    /// Ordered: arity first, because a model whose value count does not match its channel has no
    /// verdict whatever else is true of it.
    pub(crate) fn shape_refuse(&self, s: &Stmt) -> Option<FNode> {
        if let Some(node) = self.arity_refuse(s) {
            return Some(node);
        }
        let Stmt::Recv {
            channel, target, ..
        } = s
        else {
            return None;
        };
        if !self.rendezvous.contains_key(&self.channel_name(channel)) {
            return None;
        }
        match target {
            // A poll receive on a capacity-0 channel has no queue to inspect, and pan does not
            // pair it with a waiting sender either: measured, `c ? [1]` beside a sender leaves
            // the following claim refuted (pan 1, plain and -a), so the receiver waits. A step
            // that is never enabled says that, and the model is decided.
            RecvTarget::Poll(_) => Some(FNode::Blocked(
                "a poll receive waits on a channel of capacity zero",
            )),
            // A plain receive is what the handshake pairs with, and so is a random receive that
            // names values: one stager in flight makes the match a test of the staged message.
            RecvTarget::VarList(_) | RecvTarget::RandomMatch(_) => None,
            // An eval receive would have to inspect a staged message instead of taking one, and
            // the stage/release encoding has no inspect-only half.
            _ => Some(FNode::Refuse(
                "a poll or eval receive on a rendezvous channel is not implemented",
            )),
        }
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
