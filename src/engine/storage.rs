//! State storage backends for the model checker.
//!
//! Provides three storage strategies for the visited-state set:
//! - `ExactStore`: full state vectors in a hash table with collision resolution
//! - `BitstateStore`: Bloom filter with two hash functions
//! - `CollapseStore`: per-component canonical ordinals (placeholder)

use std::collections::HashMap;
use std::hash::Hash;

/// Trait abstracting state storage (visited set).
pub trait StateStore<S> {
    /// Insert a state. Returns `true` if the state was newly inserted (not a duplicate).
    fn insert(&mut self, hash: u64, state: &S) -> bool;
    /// Number of stored states.
    fn len(&self) -> usize;
    /// Check if store is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Exact state storage with collision resolution.
pub struct ExactStore<S> {
    map: HashMap<u64, Vec<S>>,
    count: usize,
}

impl<S: Clone + Hash + Eq + Send> Default for ExactStore<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Clone + Hash + Eq + Send> ExactStore<S> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
        }
    }
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for ExactStore<S> {
    fn insert(&mut self, hash: u64, state: &S) -> bool {
        let bucket = self.map.entry(hash).or_default();
        if bucket.iter().any(|s| s == state) {
            return false; // duplicate
        }
        bucket.push(state.clone());
        self.count += 1;
        true
    }

    fn len(&self) -> usize {
        self.count
    }
}

/// Bitstate (Bloom filter) storage — fixed-size bitset with two hash functions.
pub struct BitstateStore {
    bits: Vec<u8>,
    bit_count: usize,
    set_count: usize,
}

impl BitstateStore {
    /// Create a bitstate store with `size` bytes.
    pub fn new(size: usize) -> Self {
        Self {
            bits: vec![0u8; size],
            bit_count: size * 8,
            set_count: 0,
        }
    }

    fn hash_index(hash: u64, offset: u64, modulo: usize) -> usize {
        let h = hash.wrapping_mul(offset);
        (h as usize) % modulo
    }
}

impl<S> StateStore<S> for BitstateStore {
    fn insert(&mut self, _hash: u64, _state: &S) -> bool {
        // Use two independent hash positions: h1 = hash, h2 = hash.wrapping_mul(0x9e3779b9)
        let h1 = Self::hash_index(_hash, 1, self.bit_count);
        let h2 = Self::hash_index(_hash, 0x9e3779b97f4a7c15, self.bit_count);

        let already_set = {
            let byte1 = h1 / 8;
            let bit1 = h1 % 8;
            let byte2 = h2 / 8;
            let bit2 = h2 % 8;
            (self.bits[byte1] & (1 << bit1)) != 0 && (self.bits[byte2] & (1 << bit2)) != 0
        };

        if already_set {
            return false;
        }

        // Set both bits
        {
            let byte1 = h1 / 8;
            let bit1 = h1 % 8;
            let byte2 = h2 / 8;
            let bit2 = h2 % 8;
            self.bits[byte1] |= 1 << bit1;
            self.bits[byte2] |= 1 << bit2;
        }
        self.set_count += 1;
        true
    }

    fn len(&self) -> usize {
        self.set_count
    }
}

/// Collapse compression storage: canonical ordinals per component.
pub struct CollapseStore<S> {
    canonical: HashMap<u64, Vec<usize>>,
    _component_maps: Vec<HashMap<Vec<u8>, usize>>,
    count: usize,
    _marker: std::marker::PhantomData<S>,
}

impl<S: Clone + Hash + Eq + Send> CollapseStore<S> {
    pub fn new(components: usize) -> Self {
        Self {
            canonical: HashMap::new(),
            _component_maps: (0..components).map(|_| HashMap::new()).collect(),
            count: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for CollapseStore<S> {
    fn insert(&mut self, _hash: u64, _state: &S) -> bool {
        // For v1, fall through to exact-like behavior.
        // Full collapse will serialize each component separately.
        let bucket = self.canonical.entry(_hash).or_default();
        let exists = !bucket.is_empty();
        if !exists {
            bucket.push(self.count);
            self.count += 1;
            true
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_store_collision() {
        let mut store = ExactStore::new();
        assert!(store.insert(42, &"state_a".to_string()));
        assert!(store.insert(42, &"state_b".to_string()));
        assert!(!store.insert(42, &"state_a".to_string()));
        assert!(!store.insert(42, &"state_b".to_string()));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_bitstate_store() {
        let mut store = BitstateStore::new(256);
        assert!(store.insert(0, &"hello".to_string()));
        assert!(!store.insert(0, &"hello".to_string()));
        assert!(store.insert(1, &"world".to_string()));
    }

    #[test]
    fn test_exact_store_empty() {
        let store: ExactStore<String> = ExactStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }
}
