//! Emitting the two halves of a rendezvous handshake.
//!
//! Split from the step emitter so that file stays a layout: a send stages a message and waits, a
//! receive takes the staged message and releases the stager.

use super::*;

impl LuaGenerator {
    /// Stage a message and stay at this step, so the send completes only when a receiver takes it.
    pub(crate) fn emit_rdv_send(
        &mut self,
        i: usize,
        step_var: &str,
        channel: &str,
        values: &[String],
        next: usize,
    ) {
        self.emit(&format!(
            "    -- step {}: rendezvous send on {}",
            i, channel
        ));
        self.emit(&format!(
            "    guard = function(s) return s.{} == {} and not _spin_chan_staged(s, '{}') end,",
            step_var, i, channel
        ));
        let mut args = vec![
            "s".to_string(),
            format!("'{}'", channel),
            format!("'{}'", self.rendezvous_owner(channel)),
            "_spin_pid".to_string(),
            next.to_string(),
        ];
        args.extend(values.iter().cloned());
        self.emit(&format!(
            "    effect = function(s) _spin_chan_stage({}) end,",
            args.join(", ")
        ));
        // It reads the channel and writes it; it does not touch the program counter, because
        // completing the send is the receiver's doing.
        let mut deps = self.deps_of(None, &[], &[]);
        deps.write_field(channel.to_string());
        deps.reads.insert(channel.to_string());
        self.emit_deps(&deps);
        self.emit(&format!("    label = \"body:step{}\",", i));
    }

    /// Take a staged message, bind the variables, advance, and release the stager.
    pub(crate) fn emit_rdv_recv(
        &mut self,
        i: usize,
        step_var: &str,
        channel: &str,
        vars: &[String],
        matches: &[String],
        next: usize,
    ) {
        self.emit(&format!(
            "    -- step {}: rendezvous receive on {}",
            i, channel
        ));
        // A random receive that names values pairs only with a stager offering those values: spin
        // picks a matching message, and one stager in flight makes that a guard.
        let mut guard = format!(
            "s.{} == {} and _spin_chan_staged(s, '{}')",
            step_var, i, channel
        );
        for (k, want) in matches.iter().enumerate() {
            guard.push_str(&format!(
                " and _spin_chan_staged_value(s, '{}', {}) == ({})",
                channel,
                k + 1,
                want
            ));
        }
        self.emit(&format!("    guard = function(s) return {} end,", guard));
        let mut effect = format!("local _m = _spin_chan_pair(s, '{}'); ", channel);
        for (k, var) in vars.iter().enumerate() {
            effect.push_str(&format!("{} = _m.values[{}]; ", var, k + 1));
        }
        effect.push_str(&format!("s.{} = {}; ", step_var, next));
        // Release the stager: its continuation travelled in the message, and the stager's program
        // counter is the only thing that makes the send complete.
        for (prefix, instances) in self.rendezvous_senders(channel) {
            for instance in 0..instances {
                let stager = if instances == 1 {
                    prefix.clone()
                } else {
                    format!("{}{}", prefix, instance)
                };
                effect.push_str(&format!(
                    "if _m.owner == '{}' and _m.pid == {} then s._step_{} = _m.cont end; ",
                    prefix.trim_end_matches('_'),
                    instance,
                    stager
                ));
            }
        }
        self.emit(&format!("    effect = function(s) {} end,", effect));
        let mut deps = self.deps_of(None, &[], &[step_var]);
        deps.reads.insert(channel.to_string());
        deps.write_field(channel.to_string());
        for var in vars {
            if let Some(field) = var.strip_prefix("s.") {
                deps.write_field(field.to_string());
            }
        }
        // The stager's counter is written here, which no dependency model of one process can see;
        // declared so reduction cannot prune the handshake.
        for (prefix, instances) in self.rendezvous_senders(channel) {
            for instance in 0..instances {
                let stager = if instances == 1 {
                    prefix.clone()
                } else {
                    format!("{}{}", prefix, instance)
                };
                deps.write_field(format!("_step_{}", stager));
            }
        }
        self.emit_deps(&deps);
        self.emit(&format!("    label = \"body:step{}\",", i));
    }
}
