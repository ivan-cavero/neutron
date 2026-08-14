//! Small insertion/access LRU used by the worldgen ready-chunk cache.
//!
//! Not a general cache. Keys are chunk XZ; values are `Arc` encoded columns.
//! `get` promotes to most-recent so chunks around the player stay loaded.
//! Insert evicts the least-recent key when over capacity.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

/// Bounded map that evicts the least-recently-used key.
pub struct LruCache<K, V> {
    map: HashMap<K, V>,
    order: VecDeque<K>,
    cap: usize,
}

impl<K: Clone + Eq + Hash, V> LruCache<K, V> {
    /// Empty cache that holds at most `cap` entries (`cap` is clamped to ≥ 1).
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Number of live entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether `key` is present (does not change recency).
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Borrow a value and mark it most-recent.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.map.contains_key(key) {
            self.touch(key);
            self.map.get(key)
        } else {
            None
        }
    }

    /// Clone a value and mark it most-recent.
    pub fn get_cloned(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        if self.map.contains_key(key) {
            self.touch(key);
            self.map.get(key).cloned()
        } else {
            None
        }
    }

    /// Insert `key`, evicting the least-recent entry if the cap is full.
    pub fn insert(&mut self, key: K, value: V) {
        if self.map.contains_key(&key) {
            self.map.insert(key.clone(), value);
            self.touch(&key);
            return;
        }
        while self.map.len() >= self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            } else {
                break;
            }
        }
        self.order.push_back(key.clone());
        self.map.insert(key, value);
    }

    fn touch(&mut self, key: &K) {
        if let Some(i) = self.order.iter().position(|k| k == key) {
            self.order.remove(i);
        }
        self.order.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recent() {
        let mut c = LruCache::new(2);
        c.insert(1, "a");
        c.insert(2, "b");
        assert_eq!(c.get(&1), Some(&"a"));
        c.insert(3, "c");
        assert!(c.contains_key(&1));
        assert!(!c.contains_key(&2));
        assert!(c.contains_key(&3));
    }
}
