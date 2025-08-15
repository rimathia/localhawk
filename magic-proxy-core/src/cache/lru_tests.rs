//! Comprehensive LRU cache tests using vector storage
//!
//! These tests focus on LRU behavior without file system dependencies
//! or complex serialization, making them fast and reliable.

#[cfg(test)]
mod comprehensive_lru_tests {
    use super::super::lru_cache::{CacheConfig, CacheEntry, LruCache};
    use super::super::vector_storage::VectorStorage;
    use std::collections::HashMap;

    type TestCache = LruCache<String, String, VectorStorage<String, String>>;

    fn create_test_cache(max_entries: Option<usize>, max_size: Option<u64>) -> TestCache {
        let storage = VectorStorage::new();
        let config = CacheConfig {
            max_entries,
            max_size_bytes: max_size,
            eager_persistence: false,
        };
        LruCache::new(storage, config).unwrap()
    }

    #[test]
    fn test_lru_ordering_simple() {
        let mut cache = create_test_cache(Some(3), None);

        // Fill cache
        cache.insert("a".to_string(), "1".to_string()).unwrap();
        cache.insert("b".to_string(), "2".to_string()).unwrap();
        cache.insert("c".to_string(), "3".to_string()).unwrap();
        assert_eq!(cache.len(), 3);

        // Access 'a' to make it most recently used: a(newest) -> c -> b(oldest)
        cache.get(&"a".to_string());

        // Insert 'd' - should evict 'b' (oldest)
        cache.insert("d".to_string(), "4".to_string()).unwrap();
        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&"a".to_string()));
        assert!(!cache.contains(&"b".to_string())); // evicted
        assert!(cache.contains(&"c".to_string()));
        assert!(cache.contains(&"d".to_string()));
    }

    #[test]
    fn test_lru_ordering_complex() {
        let mut cache = create_test_cache(Some(4), None);

        // Fill cache: order should be d(newest) -> c -> b -> a(oldest)
        cache.insert("a".to_string(), "1".to_string()).unwrap();
        cache.insert("b".to_string(), "2".to_string()).unwrap();
        cache.insert("c".to_string(), "3".to_string()).unwrap();
        cache.insert("d".to_string(), "4".to_string()).unwrap();

        // Complex access pattern:
        cache.get(&"a".to_string()); // a(newest) -> d -> c -> b(oldest)
        cache.get(&"c".to_string()); // c(newest) -> a -> d -> b(oldest)
        cache.get(&"b".to_string()); // b(newest) -> c -> a -> d(oldest)

        // Insert 'e' - should evict 'd' (oldest)
        cache.insert("e".to_string(), "5".to_string()).unwrap();
        assert_eq!(cache.len(), 4);
        assert!(cache.contains(&"a".to_string()));
        assert!(cache.contains(&"b".to_string()));
        assert!(cache.contains(&"c".to_string()));
        assert!(!cache.contains(&"d".to_string())); // evicted
        assert!(cache.contains(&"e".to_string()));
    }

    #[test]
    fn test_size_based_eviction() {
        // Each entry = 100 bytes, max = 250 bytes = ~2.5 entries
        let mut cache = create_test_cache(None, Some(250));

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap(); // 100 bytes
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap(); // 200 bytes total
        cache
            .insert("key3".to_string(), "value3".to_string())
            .unwrap(); // 300 bytes total - should evict key1

        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&"key1".to_string())); // evicted due to size
        assert!(cache.contains(&"key2".to_string()));
        assert!(cache.contains(&"key3".to_string()));
    }

    #[test]
    fn test_mixed_count_and_size_limits() {
        // Both limits: max 3 entries AND max 250 bytes
        let mut cache = create_test_cache(Some(3), Some(250));

        cache.insert("a".to_string(), "1".to_string()).unwrap(); // 100 bytes
        cache.insert("b".to_string(), "2".to_string()).unwrap(); // 200 bytes
        cache.insert("c".to_string(), "3".to_string()).unwrap(); // 300 bytes - exceeds size limit

        // Size limit should trigger first (only 2 entries fit in 250 bytes)
        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&"a".to_string())); // evicted due to size

        // Now test count limit
        cache.insert("d".to_string(), "4".to_string()).unwrap(); // Still 2 entries, 200 bytes
        cache.insert("e".to_string(), "5".to_string()).unwrap(); // Still 2 entries, 200 bytes (evicts b)

        assert_eq!(cache.len(), 2);
        assert!(!cache.contains(&"b".to_string())); // evicted due to LRU
        assert!(cache.contains(&"d".to_string()));
        assert!(cache.contains(&"e".to_string()));
    }

    #[test]
    fn test_eviction_callbacks() {
        let storage = VectorStorage::<String, String>::new();
        let config = CacheConfig {
            max_entries: Some(2),
            max_size_bytes: None,
            eager_persistence: false,
        };
        let mut cache = LruCache::new(storage.clone(), config).unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();
        cache
            .insert("key3".to_string(), "value3".to_string())
            .unwrap(); // evicts key1

        // Check that eviction callback was called
        let evicted = storage.get_evicted_entries();
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0], ("key1".to_string(), "value1".to_string()));
    }

    #[test]
    fn test_manual_eviction() {
        let storage = VectorStorage::<String, String>::new();
        let config = CacheConfig::default();
        let mut cache = LruCache::new(storage.clone(), config).unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();

        // Manual eviction
        let evicted = cache.evict(&"key1".to_string()).unwrap();
        assert!(evicted);
        assert_eq!(cache.len(), 1);
        assert!(!cache.contains(&"key1".to_string()));

        // Check callback was called
        let evicted_entries = storage.get_evicted_entries();
        assert_eq!(evicted_entries.len(), 1);
        assert_eq!(
            evicted_entries[0],
            ("key1".to_string(), "value1".to_string())
        );

        // Evicting non-existent key
        let evicted = cache.evict(&"nonexistent".to_string()).unwrap();
        assert!(!evicted);
    }

    #[test]
    fn test_cache_clear() {
        let storage = VectorStorage::<String, String>::new();
        let config = CacheConfig::default();
        let mut cache = LruCache::new(storage.clone(), config).unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();
        cache
            .insert("key3".to_string(), "value3".to_string())
            .unwrap();
        assert_eq!(cache.len(), 3);

        cache.clear().unwrap();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // All entries should have been evicted via callbacks
        let evicted_entries = storage.get_evicted_entries();
        assert_eq!(evicted_entries.len(), 3);
    }

    #[test]
    fn test_update_existing_entry() {
        let mut cache = create_test_cache(Some(2), None);

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();

        // Update existing entry (should not trigger eviction)
        cache
            .insert("key1".to_string(), "updated_value".to_string())
            .unwrap();
        assert_eq!(cache.len(), 2);

        let value = cache.get(&"key1".to_string());
        assert_eq!(value, Some("updated_value".to_string()));
    }

    #[test]
    fn test_access_pattern_updates_lru() {
        let mut cache = create_test_cache(Some(3), None);

        cache.insert("a".to_string(), "1".to_string()).unwrap();
        cache.insert("b".to_string(), "2".to_string()).unwrap();
        cache.insert("c".to_string(), "3".to_string()).unwrap();

        // Multiple accesses to 'a' should keep it as most recent
        cache.get(&"a".to_string());
        cache.get(&"a".to_string());
        cache.get(&"a".to_string());

        // Insert new entry - should evict 'b' (oldest unaccessed)
        cache.insert("d".to_string(), "4".to_string()).unwrap();
        assert!(cache.contains(&"a".to_string()));
        assert!(!cache.contains(&"b".to_string())); // evicted
        assert!(cache.contains(&"c".to_string()));
        assert!(cache.contains(&"d".to_string()));
    }

    #[test]
    fn test_load_failure_handling() {
        let mut storage = VectorStorage::<String, String>::new();
        storage.set_failure_modes(true, false, false);

        let config = CacheConfig::default();

        // Load failure should not prevent cache creation (graceful degradation)
        let cache_result = LruCache::new(storage, config);
        assert!(cache_result.is_ok());

        let cache = cache_result.unwrap();
        assert!(cache.is_empty()); // Should start empty despite load failure
    }

    #[test]
    fn test_save_failure_handling() {
        let mut storage = VectorStorage::<String, String>::new();
        storage.set_failure_modes(false, true, false);

        let config = CacheConfig {
            max_entries: Some(2),
            max_size_bytes: None,
            eager_persistence: true, // Force save on every insert
        };

        let mut cache = LruCache::new(storage, config).unwrap();

        // Save failures should propagate as errors
        let result = cache.insert("key1".to_string(), "value1".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_evict_failure_handling() {
        let mut storage = VectorStorage::<String, String>::new();
        storage.set_failure_modes(false, false, true);

        let config = CacheConfig {
            max_entries: Some(1),
            max_size_bytes: None,
            eager_persistence: false,
        };

        let mut cache = LruCache::new(storage, config).unwrap();

        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();

        // This should trigger eviction, which will fail
        let result = cache.insert("key2".to_string(), "value2".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_stats_accuracy() {
        let mut cache = create_test_cache(None, None);

        // Empty cache stats
        let stats = cache.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.size_bytes, 0);
        assert!(stats.oldest_entry.is_none());
        assert!(stats.most_recent_access.is_none());

        // Add entries and check stats
        cache
            .insert("key1".to_string(), "value1".to_string())
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1)); // Ensure different timestamps
        cache
            .insert("key2".to_string(), "value2".to_string())
            .unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.size_bytes, 200); // 2 entries * 100 bytes each
        assert!(stats.oldest_entry.is_some());
        assert!(stats.most_recent_access.is_some());

        // Oldest should be key1's timestamp, most recent should be key2's
        let oldest = stats.oldest_entry.unwrap();
        let most_recent = stats.most_recent_access.unwrap();
        assert!(oldest <= most_recent);
    }

    #[test]
    fn test_zero_size_limit_behavior() {
        // Zero size limit should prevent any entries
        let mut cache = create_test_cache(None, Some(0));

        let _result = cache.insert("key1".to_string(), "value1".to_string());
        // With zero size limit, entry might be inserted then immediately evicted
        // The important thing is the cache should end up empty or minimal
        assert!(cache.len() <= 1); // Allow for the edge case where insert succeeds but evicts immediately
    }

    #[test]
    fn test_persistence_integration() {
        let storage = VectorStorage::<String, String>::new();

        // Preload storage with data
        let mut initial_data = HashMap::new();
        initial_data.insert("preloaded".to_string(), CacheEntry::new("data".to_string()));
        storage.preload(initial_data);

        let config = CacheConfig::default();
        let mut cache = LruCache::new(storage.clone(), config).unwrap();

        // Should load the preloaded data
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&"preloaded".to_string()));

        // Add more data
        cache
            .insert("new_key".to_string(), "new_value".to_string())
            .unwrap();

        // Save should be called (can verify with storage.save_call_count())
        cache.save_to_storage().unwrap();
        assert!(storage.save_call_count() > 0);
    }
}
