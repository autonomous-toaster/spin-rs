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

/// Hash-compact storage: stores 64-bit hashes with an LRU cache for collision detection.
///
/// On collision (same hash, different state), falls back to exact storage for that state.
pub struct HashCompactStore<S> {
    /// Hash table: hash -> (state_count, fallback_to_exact)
    hash_table: HashMap<u64, (u32, bool)>,
    /// LRU cache of recent full states for collision detection
    lru_cache: std::collections::VecDeque<(u64, S)>,
    /// Maximum size of the LRU cache
    lru_max_size: usize,
    /// Fallback exact store for states that had collisions
    fallback: ExactStore<S>,
    /// Total count of stored states
    count: usize,
}

impl<S: Clone + Hash + Eq + Send> HashCompactStore<S> {
    /// Create a new hash-compact store with the given LRU cache size.
    pub fn new(lru_size: usize) -> Self {
        Self {
            hash_table: HashMap::new(),
            lru_cache: std::collections::VecDeque::with_capacity(lru_size),
            lru_max_size: lru_size,
            fallback: ExactStore::new(),
            count: 0,
        }
    }

    /// Check if a state matches a cached entry (collision detection).
    fn check_cache(&self, hash: u64, state: &S) -> Option<bool> {
        for (cached_hash, cached_state) in &self.lru_cache {
            if *cached_hash == hash && cached_state == state {
                return Some(true);
            }
        }
        None
    }

    /// Add a state to the LRU cache.
    fn add_to_cache(&mut self, hash: u64, state: &S) {
        if self.lru_cache.len() >= self.lru_max_size {
            self.lru_cache.pop_front();
        }
        self.lru_cache.push_back((hash, state.clone()));
    }
}

impl<S: Clone + Hash + Eq + Send> StateStore<S> for HashCompactStore<S> {
    fn insert(&mut self, hash: u64, state: &S) -> bool {
        if let Some((count, fallback)) = self.hash_table.get(&hash) {
            if *fallback {
                return self.fallback.insert(hash, state);
            }

            if let Some(found) = self.check_cache(hash, state)
                && found {
                    return false;
                }

            // Hash collision detected: same hash, different state
            self.hash_table.insert(hash, (*count, true));
            let cached_states: Vec<S> = self.lru_cache.iter()
                .filter(|(h, _)| *h == hash)
                .map(|(_, s)| s.clone())
                .collect();
            for s in cached_states {
                self.fallback.insert(hash, &s);
            }
            self.fallback.insert(hash, state);
            self.count += 1;
            return true;
        }

        // New hash — store it
        self.hash_table.insert(hash, (1, false));
        self.add_to_cache(hash, state);
        self.count += 1;
        true
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

    #[test]
    fn test_hash_compact_store_basic() {
        let mut store = HashCompactStore::new(16);
        assert!(store.insert(1, &"state_a".to_string()));
        assert!(!store.insert(1, &"state_a".to_string()));
        assert!(store.insert(2, &"state_b".to_string()));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_hash_compact_collision_fallback() {
        let mut store = HashCompactStore::new(16);
        // Same hash, different states -> collision -> fallback to exact
        assert!(store.insert(42, &"state_a".to_string()));
        assert!(store.insert(42, &"state_b".to_string()));
        // After collision, both should be stored
        assert!(!store.insert(42, &"state_a".to_string()));
        assert!(!store.insert(42, &"state_b".to_string()));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_hash_compact_matches_exact() {
        let mut exact = ExactStore::new();
        let mut hc = HashCompactStore::new(1024);

        let states = vec!["a", "b", "c", "d", "e"];
        for (i, s) in states.iter().enumerate() {
            let hash = i as u64;
            assert_eq!(exact.insert(hash, &s.to_string()), hc.insert(hash, &s.to_string()));
        }

        // Re-insert all (should be duplicates)
        for (i, s) in states.iter().enumerate() {
            let hash = i as u64;
            assert_eq!(exact.insert(hash, &s.to_string()), hc.insert(hash, &s.to_string()));
        }

        assert_eq!(exact.len(), hc.len());
    }

    #[test]
    fn test_hash_compact_lru_eviction() {
        let mut store = HashCompactStore::new(4); // Small LRU cache
        for i in 0..10u64 {
            assert!(store.insert(i, &format!("state_{}", i)));
        }
        // All 10 states should be stored (LRU only affects collision detection, not storage)
        assert_eq!(store.len(), 10);
    }
}
