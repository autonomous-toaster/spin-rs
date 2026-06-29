#[derive(Debug, Clone)]
pub struct LuaChannel {
    pub(crate) capacity: usize,
    pub(crate) messages: Vec<Vec<i64>>,
}

impl LuaChannel {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            messages: Vec::new(),
        }
    }

    pub fn send(&mut self, msg: Vec<i64>) -> bool {
        // Rendezvous channels (capacity 0): send always blocks
        if self.capacity == 0 {
            return false;
        }
        if self.messages.len() >= self.capacity {
            return false;
        }
        self.messages.push(msg);
        true
    }

    pub fn recv(&mut self) -> Option<Vec<i64>> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.messages.remove(0))
        }
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
    pub fn is_full(&self) -> bool {
        // Rendezvous channels (capacity 0): full only if a message is waiting (blocked sender)
        // Buffered channels: full if at capacity
        self.capacity == 0 && !self.messages.is_empty()
            || (self.capacity > 0 && self.messages.len() >= self.capacity)
    }
    pub fn is_empty(&self) -> bool {
        // Both rendezvous and buffered: empty if no messages waiting
        self.messages.is_empty()
    }

    /// Send a message in sorted order (ascending by first element).
    /// Inserts at the position that maintains sorted order.
    pub fn send_sorted(&mut self, msg: Vec<i64>) -> bool {
        if self.capacity == 0 {
            return false; // Rendezvous: always blocks
        }
        if self.messages.len() >= self.capacity {
            return false;
        }
        let val = msg.first().copied().unwrap_or(0);
        let pos = self
            .messages
            .iter()
            .position(|m| m.first().copied().unwrap_or(0) > val)
            .unwrap_or(self.messages.len());
        self.messages.insert(pos, msg);
        true
    }

    /// Receive a random message (non-deterministically pick any message).
    /// Uses the first message as a simple heuristic (true non-determinism
    /// requires the model checker to explore all possibilities).
    pub fn recv_random(&mut self) -> Option<Vec<i64>> {
        if self.messages.is_empty() {
            return None;
        }
        // Remove from a random position (first for simplicity;
        // the model checker explores all interleavings)
        Some(self.messages.remove(0))
    }

    /// Poll: check if the first message matches a condition without consuming.
    /// Returns Some(value) if the first message matches, None otherwise.
    /// The message is NOT removed from the channel.
    pub fn poll(&self, expected: i64) -> Option<i64> {
        self.messages.first().and_then(|msg| {
            let val = msg.first().copied().unwrap_or(0);
            if val == expected { Some(val) } else { None }
        })
    }

    /// Receive only if the first message matches a given value (eval receive).
    /// Returns Some(value) and removes the message if it matches,
    /// None if the first message doesn't match or channel is empty.
    pub fn recv_eval(&mut self, expected: i64) -> Option<Vec<i64>> {
        if self.messages.is_empty() {
            return None;
        }
        let val = self.messages[0].first().copied().unwrap_or(0);
        if val == expected {
            Some(self.messages.remove(0))
        } else {
            None
        }
    }
}
