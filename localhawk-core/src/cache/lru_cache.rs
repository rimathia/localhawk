//! Generic LRU Cache Framework
//!
//! This module provides a generic LRU (Least Recently Used) cache with pluggable storage strategies.
//! The cache supports both entry count limits and total size limits, with automatic eviction of
//! least recently used items when limits are exceeded.

use crate::error::ProxyError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use time::OffsetDateTime;
use tracing::{debug, info, warn};

/// A cache entry with access tracking for LRU eviction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<V> {
    pub value: V,
    pub created_at: OffsetDateTime,
    pub last_accessed: OffsetDateTime,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            value,
            created_at: now,
            last_accessed: now,
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = OffsetDateTime::now_utc();
    }
}

/// Storage strategy trait for pluggable cache persistence
pub trait StorageStrategy<K, V>: Send + Sync
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Load all cache entries from persistent storage
    fn load(&self) -> Result<HashMap<K, CacheEntry<V>>, ProxyError>;

    /// Save all cache entries to persistent storage
    fn save(&self, entries: &HashMap<K, CacheEntry<V>>) -> Result<(), ProxyError>;

    /// Estimate the size in bytes of a cache entry (key + value + metadata)
    fn estimate_size(&self, key: &K, value: &V) -> u64;

    /// Get the fixed size estimate per entry for O(1) size calculations
    fn get_size_estimate(&self) -> u64;

    /// Called when an entry is evicted from the cache (for cleanup)
    fn evict_entry(&self, key: &K, value: &V) -> Result<(), ProxyError>;

    /// Get a human-readable name for this storage strategy (for logging)
    fn strategy_name(&self) -> &'static str;
}

/// Configuration for LRU cache limits
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries (None = unlimited)
    pub max_entries: Option<usize>,
    /// Maximum total size in bytes (None = unlimited)
    pub max_size_bytes: Option<u64>,
    /// Whether to save to disk on every insert (vs only on shutdown)
    pub eager_persistence: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: Some(1000), // Reasonable default
            max_size_bytes: None,
            eager_persistence: false,
        }
    }
}

/// Generic LRU cache with pluggable storage
pub struct LruCache<K, V, S>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: StorageStrategy<K, V>,
{
    entries: HashMap<K, CacheEntry<V>>,
    storage: S,
    config: CacheConfig,
}

impl<K, V, S> LruCache<K, V, S>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: StorageStrategy<K, V>,
{
    /// Create a new LRU cache with the given storage strategy and configuration
    pub fn new(storage: S, config: CacheConfig) -> Result<Self, ProxyError> {
        let mut cache = Self {
            entries: HashMap::new(),
            storage,
            config,
        };

        // Load existing data from storage
        cache.load_from_storage()?;

        Ok(cache)
    }

    /// Get a value from the cache, updating its access time
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.touch();
            debug!(strategy = %self.storage.strategy_name(), "Cache HIT");
            Some(entry.value.clone())
        } else {
            debug!(strategy = %self.storage.strategy_name(), "Cache MISS");
            None
        }
    }

    /// Insert a value into the cache, potentially evicting old entries
    pub fn insert(&mut self, key: K, value: V) -> Result<(), ProxyError> {
        // Check if we need to make space first
        self.ensure_space_for_new_entry(&key, &value)?;

        // Insert the new entry
        let entry = CacheEntry::new(value.clone());
        self.entries.insert(key.clone(), entry);

        debug!(
            strategy = %self.storage.strategy_name(),
            entries = self.entries.len(),
            "Inserted cache entry"
        );

        // Save to storage if eager persistence is enabled
        if self.config.eager_persistence {
            self.save_to_storage()?;
        }

        Ok(())
    }

    /// Check if the cache contains a key
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Get the number of entries in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the total estimated size of the cache in bytes
    pub fn size_bytes(&self) -> u64 {
        (self.entries.len() as u64) * self.storage.get_size_estimate()
    }

    /// Force evict a specific entry
    pub fn evict(&mut self, key: &K) -> Result<bool, ProxyError> {
        if let Some(entry) = self.entries.remove(key) {
            self.storage.evict_entry(key, &entry.value)?;
            debug!(strategy = %self.storage.strategy_name(), "Force evicted cache entry");

            if self.config.eager_persistence {
                self.save_to_storage()?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) -> Result<(), ProxyError> {
        // Notify storage of all evictions
        for (key, entry) in &self.entries {
            self.storage.evict_entry(key, &entry.value)?;
        }

        self.entries.clear();
        self.save_to_storage()?;

        info!(strategy = %self.storage.strategy_name(), "Cleared all cache entries");
        Ok(())
    }

    /// Save the current cache state to storage
    pub fn save_to_storage(&self) -> Result<(), ProxyError> {
        self.storage.save(&self.entries)?;
        debug!(
            strategy = %self.storage.strategy_name(),
            entries = self.entries.len(),
            "Saved cache to storage"
        );
        Ok(())
    }

    /// Load cache state from storage
    fn load_from_storage(&mut self) -> Result<(), ProxyError> {
        match self.storage.load() {
            Ok(entries) => {
                self.entries = entries;
                info!(
                    strategy = %self.storage.strategy_name(),
                    entries = self.entries.len(),
                    "Loaded cache from storage"
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    strategy = %self.storage.strategy_name(),
                    error = %e,
                    "Failed to load cache from storage, starting empty"
                );
                self.entries.clear();
                Ok(())
            }
        }
    }

    /// Ensure there's space for a new entry, evicting old ones if necessary
    fn ensure_space_for_new_entry(&mut self, new_key: &K, new_value: &V) -> Result<(), ProxyError> {
        let new_entry_size = self.storage.estimate_size(new_key, new_value);

        // Check entry count limit
        if let Some(max_entries) = self.config.max_entries {
            if self.entries.len() >= max_entries && !self.entries.contains_key(new_key) {
                self.evict_lru_entries(1, 0)?;
            }
        }

        // Check size limit
        if let Some(max_size) = self.config.max_size_bytes {
            let current_size = self.size_bytes();
            if current_size + new_entry_size > max_size {
                let size_to_free = (current_size + new_entry_size) - max_size;
                self.evict_lru_entries(0, size_to_free)?;
            }
        }

        Ok(())
    }

    /// Evict least recently used entries to free up space
    fn evict_lru_entries(&mut self, min_count: usize, min_size: u64) -> Result<(), ProxyError> {
        // Sort entries by last access time (oldest first)
        let mut entries_by_access: Vec<_> = self
            .entries
            .iter()
            .map(|(key, entry)| (key.clone(), entry.last_accessed))
            .collect();

        entries_by_access.sort_by_key(|(_, last_accessed)| *last_accessed);

        let mut evicted_count = 0;
        let mut size_freed = 0u64;
        let mut keys_to_remove = Vec::new();

        for (key, _) in entries_by_access {
            if evicted_count >= min_count && size_freed >= min_size {
                break;
            }

            if let Some(entry) = self.entries.get(&key) {
                size_freed += self.storage.estimate_size(&key, &entry.value);
                keys_to_remove.push(key);
                evicted_count += 1;
            }
        }

        // Actually remove the entries
        for key in keys_to_remove {
            if let Some(entry) = self.entries.remove(&key) {
                self.storage.evict_entry(&key, &entry.value)?;
            }
        }

        if evicted_count > 0 {
            info!(
                strategy = %self.storage.strategy_name(),
                evicted_count = evicted_count,
                size_freed_kb = size_freed / 1024,
                "Evicted LRU entries"
            );
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entries.len(),
            size_bytes: self.size_bytes(),
            oldest_entry: self.entries.values().map(|entry| entry.created_at).min(),
            most_recent_access: self.entries.values().map(|entry| entry.last_accessed).max(),
        }
    }
}

/// Cache statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub size_bytes: u64,
    pub oldest_entry: Option<OffsetDateTime>,
    pub most_recent_access: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock storage strategy for testing
    struct MockStorage {
        pub should_fail_load: bool,
        pub save_calls: std::sync::Arc<std::sync::Mutex<Vec<HashMap<String, CacheEntry<String>>>>>,
        pub evict_calls: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                should_fail_load: false,
                save_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                evict_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
    }

    impl StorageStrategy<String, String> for MockStorage {
        fn load(&self) -> Result<HashMap<String, CacheEntry<String>>, ProxyError> {
            if self.should_fail_load {
                Err(ProxyError::Cache("Mock load failure".to_string()))
            } else {
                Ok(HashMap::new())
            }
        }

        fn save(&self, entries: &HashMap<String, CacheEntry<String>>) -> Result<(), ProxyError> {
            self.save_calls.lock().unwrap().push(entries.clone());
            Ok(())
        }

        fn estimate_size(&self, key: &String, value: &String) -> u64 {
            (key.len() + value.len()) as u64
        }

        fn get_size_estimate(&self) -> u64 {
            10 // Fixed estimate for test consistency
        }

        fn evict_entry(&self, key: &String, value: &String) -> Result<(), ProxyError> {
            self.evict_calls
                .lock()
                .unwrap()
                .push((key.clone(), value.clone()));
            Ok(())
        }

        fn strategy_name(&self) -> &'static str {
            "MockStorage"
        }
    }

    #[test]
    fn test_basic_cache_operations() {
        let storage = MockStorage::new();
        let config = CacheConfig::default();
        let mut cache = LruCache::new(storage, config).unwrap();

        // Initially empty
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Insert and retrieve
        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&"key1".to_string()));

        let retrieved = cache.get(&"key1".to_string());
        assert_eq!(retrieved, Some("value1".to_string()));
    }

    #[test]
    fn test_lru_eviction_by_count() {
        let storage = MockStorage::new();
        let config = CacheConfig {
            max_entries: Some(2),
            max_size_bytes: None,
            eager_persistence: false,
        };
        let mut cache = LruCache::new(storage, config).unwrap();

        // Fill cache to limit
        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();
        assert_eq!(cache.len(), 2);

        // Access key1 to make it more recently used
        cache.get(&"key1".to_string());

        // Insert key3 - should evict key2 (least recently used)
        cache
            .insert("key3".to_string(), "value3".to_string())
            .unwrap();
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&"key1".to_string()));
        assert!(!cache.contains(&"key2".to_string()));
        assert!(cache.contains(&"key3".to_string()));
    }

    #[test]
    fn test_cache_stats() {
        let storage = MockStorage::new();
        let config = CacheConfig::default();
        let mut cache = LruCache::new(storage, config).unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 1);
        assert!(stats.size_bytes > 0);
        assert!(stats.oldest_entry.is_some());
        assert!(stats.most_recent_access.is_some());
    }
}
