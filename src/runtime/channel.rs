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
}
