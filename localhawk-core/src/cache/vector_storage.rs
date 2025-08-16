//! Vector-based storage strategy for simple unit testing
//!
//! This storage strategy stores all operations in memory vectors,
//! making it perfect for testing LRU cache behavior without
//! file system dependencies or complex serialization.

use super::lru_cache::{CacheEntry, StorageStrategy};
use crate::error::ProxyError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Simple vector-based storage for testing
/// All operations are recorded in vectors for inspection
#[derive(Debug, Clone)]
pub struct VectorStorage<K, V> {
    /// In-memory storage (simulates persistent storage)
    storage: Arc<Mutex<HashMap<K, CacheEntry<V>>>>,
    /// Record of all load() calls
    pub load_calls: Arc<Mutex<Vec<usize>>>, // Records number of entries loaded each time
    /// Record of all save() calls  
    pub save_calls: Arc<Mutex<Vec<usize>>>, // Records number of entries saved each time
    /// Record of all evict_entry() calls
    pub evict_calls: Arc<Mutex<Vec<(K, V)>>>, // Records (key, value) of evicted entries
    /// Control behavior for testing
    pub should_fail_load: bool,
    pub should_fail_save: bool,
    pub should_fail_evict: bool,
    /// Size estimate per entry
    size_per_entry: u64,
}

impl<K, V> VectorStorage<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    /// Create a new vector storage with default size estimate
    pub fn new() -> Self {
        Self::with_size_estimate(100)
    }

    /// Create a new vector storage with custom size estimate per entry
    pub fn with_size_estimate(size_per_entry: u64) -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
            load_calls: Arc::new(Mutex::new(Vec::new())),
            save_calls: Arc::new(Mutex::new(Vec::new())),
            evict_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail_load: false,
            should_fail_save: false,
            should_fail_evict: false,
            size_per_entry,
        }
    }

    /// Pre-populate storage with test data (simulates existing cache)
    pub fn preload(&self, data: HashMap<K, CacheEntry<V>>) {
        let mut storage = self.storage.lock().unwrap();
        *storage = data;
    }

    /// Get current storage contents (for test assertions)
    pub fn get_storage(&self) -> HashMap<K, CacheEntry<V>> {
        self.storage.lock().unwrap().clone()
    }

    /// Check how many times load() was called
    pub fn load_call_count(&self) -> usize {
        self.load_calls.lock().unwrap().len()
    }

    /// Check how many times save() was called
    pub fn save_call_count(&self) -> usize {
        self.save_calls.lock().unwrap().len()
    }

    /// Check how many evictions happened
    pub fn evict_call_count(&self) -> usize {
        self.evict_calls.lock().unwrap().len()
    }

    /// Get all evicted entries (for verifying LRU behavior)
    pub fn get_evicted_entries(&self) -> Vec<(K, V)> {
        self.evict_calls.lock().unwrap().clone()
    }

    /// Reset all recorded calls (for test isolation)
    pub fn reset_calls(&self) {
        self.load_calls.lock().unwrap().clear();
        self.save_calls.lock().unwrap().clear();
        self.evict_calls.lock().unwrap().clear();
    }

    /// Configure failure modes for testing error handling
    pub fn set_failure_modes(&mut self, load: bool, save: bool, evict: bool) {
        self.should_fail_load = load;
        self.should_fail_save = save;
        self.should_fail_evict = evict;
    }
}

impl<K, V> Default for VectorStorage<K, V>
where
    K: std::hash::Hash + Eq + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> StorageStrategy<K, V> for VectorStorage<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
{
    fn load(&self) -> Result<HashMap<K, CacheEntry<V>>, ProxyError> {
        if self.should_fail_load {
            return Err(ProxyError::Cache("Simulated load failure".to_string()));
        }

        let storage = self.storage.lock().unwrap();
        let entry_count = storage.len();

        // Record the load call
        self.load_calls.lock().unwrap().push(entry_count);

        Ok(storage.clone())
    }

    fn save(&self, entries: &HashMap<K, CacheEntry<V>>) -> Result<(), ProxyError> {
        if self.should_fail_save {
            return Err(ProxyError::Cache("Simulated save failure".to_string()));
        }

        // Record the save call
        self.save_calls.lock().unwrap().push(entries.len());

        // Update storage
        let mut storage = self.storage.lock().unwrap();
        *storage = entries.clone();

        Ok(())
    }

    fn estimate_size(&self, _key: &K, _value: &V) -> u64 {
        self.size_per_entry
    }

    fn get_size_estimate(&self) -> u64 {
        self.size_per_entry
    }

    fn evict_entry(&self, key: &K, value: &V) -> Result<(), ProxyError> {
        if self.should_fail_evict {
            return Err(ProxyError::Cache("Simulated evict failure".to_string()));
        }

        // Record the eviction
        self.evict_calls
            .lock()
            .unwrap()
            .push((key.clone(), value.clone()));

        // Remove from storage
        let mut storage = self.storage.lock().unwrap();
        storage.remove(key);

        Ok(())
    }

    fn strategy_name(&self) -> &'static str {
        "VectorStorage"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_storage_basic() {
        let storage = VectorStorage::<String, String>::new();

        // Initially empty
        let loaded = storage.load().unwrap();
        assert!(loaded.is_empty());
        assert_eq!(storage.load_call_count(), 1);

        // Save some data
        let mut entries = HashMap::new();
        let entry = CacheEntry::new("value1".to_string());
        entries.insert("key1".to_string(), entry);

        storage.save(&entries).unwrap();
        assert_eq!(storage.save_call_count(), 1);

        // Load it back
        let reloaded = storage.load().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert!(reloaded.contains_key("key1"));
        assert_eq!(storage.load_call_count(), 2);
    }

    #[test]
    fn test_vector_storage_eviction_tracking() {
        let storage = VectorStorage::<String, String>::new();

        // Simulate eviction
        storage
            .evict_entry(&"key1".to_string(), &"value1".to_string())
            .unwrap();
        storage
            .evict_entry(&"key2".to_string(), &"value2".to_string())
            .unwrap();

        assert_eq!(storage.evict_call_count(), 2);
        let evicted = storage.get_evicted_entries();
        assert_eq!(evicted.len(), 2);
        assert_eq!(evicted[0], ("key1".to_string(), "value1".to_string()));
        assert_eq!(evicted[1], ("key2".to_string(), "value2".to_string()));
    }

    #[test]
    fn test_vector_storage_failure_modes() {
        let mut storage = VectorStorage::<String, String>::new();
        storage.set_failure_modes(true, true, true);

        // All operations should fail
        assert!(storage.load().is_err());
        assert!(storage.save(&HashMap::new()).is_err());
        assert!(
            storage
                .evict_entry(&"key".to_string(), &"value".to_string())
                .is_err()
        );
    }

    #[test]
    fn test_vector_storage_preload() {
        let storage = VectorStorage::<String, String>::new();

        // Preload with data
        let mut data = HashMap::new();
        data.insert("key1".to_string(), CacheEntry::new("value1".to_string()));
        data.insert("key2".to_string(), CacheEntry::new("value2".to_string()));

        storage.preload(data);

        // Should load the preloaded data
        let loaded = storage.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(loaded.contains_key("key1"));
        assert!(loaded.contains_key("key2"));
    }

    #[test]
    fn test_vector_storage_reset_calls() {
        let storage = VectorStorage::<String, String>::new();

        // Make some calls
        storage.load().unwrap();
        storage.save(&HashMap::new()).unwrap();
        storage
            .evict_entry(&"key".to_string(), &"value".to_string())
            .unwrap();

        assert_eq!(storage.load_call_count(), 1);
        assert_eq!(storage.save_call_count(), 1);
        assert_eq!(storage.evict_call_count(), 1);

        // Reset and verify
        storage.reset_calls();
        assert_eq!(storage.load_call_count(), 0);
        assert_eq!(storage.save_call_count(), 0);
        assert_eq!(storage.evict_call_count(), 0);
    }
}
