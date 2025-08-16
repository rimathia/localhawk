//! Search results cache implementation using the generic LRU framework

use super::search_json_storage::SearchJsonStorage;
use super::{CacheConfig, LruCache};
use crate::error::ProxyError;
use crate::scryfall::CardSearchResult;
use std::path::PathBuf;

const SEARCH_RESULT_SIZE_ESTIMATE: u64 = 50 * 1024; // 50 KB per cached search
const DEFAULT_MAX_SEARCHES: usize = 1000; // Reasonable limit for search results

/// Search results cache type alias
pub type LruSearchCache = LruCache<String, CardSearchResult, SearchJsonStorage>;

/// Create a new search results cache with sensible defaults
pub fn create_search_cache() -> Result<LruSearchCache, ProxyError> {
    let cache_file = PathBuf::from(crate::get_search_cache_path());

    let storage = SearchJsonStorage::new(cache_file, SEARCH_RESULT_SIZE_ESTIMATE)?;

    let config = CacheConfig {
        max_entries: Some(DEFAULT_MAX_SEARCHES),
        max_size_bytes: Some(DEFAULT_MAX_SEARCHES as u64 * SEARCH_RESULT_SIZE_ESTIMATE), // ~50MB max
        eager_persistence: false, // Save only on shutdown for performance
    };

    LruCache::new(storage, config)
}

/// Create a new search results cache with custom configuration
pub fn create_search_cache_with_config(
    cache_file: PathBuf,
    max_searches: usize,
) -> Result<LruSearchCache, ProxyError> {
    let storage = SearchJsonStorage::new(cache_file, SEARCH_RESULT_SIZE_ESTIMATE)?;

    let config = CacheConfig {
        max_entries: Some(max_searches),
        max_size_bytes: Some(max_searches as u64 * SEARCH_RESULT_SIZE_ESTIMATE),
        eager_persistence: false,
    };

    LruCache::new(storage, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::{Card, CardSearchResult};
    use std::env;

    fn create_test_search_result(card_name: &str, count: usize) -> CardSearchResult {
        let cards = (0..count)
            .map(|i| Card {
                name: card_name.to_string(),
                set: format!("set{}", i),
                language: "en".to_string(),
                border_crop: format!("https://example.com/image{}.jpg", i),
                back_side: None,
            })
            .collect();

        CardSearchResult {
            cards,
            total_found: count,
        }
    }

    #[test]
    fn test_search_cache_basic() {
        let temp_file = env::temp_dir().join(format!(
            "magic-proxy-search-test-{}.json",
            std::process::id()
        ));

        let mut cache = create_search_cache_with_config(temp_file.clone(), 100).unwrap();

        // Test basic operations
        assert!(cache.is_empty());

        let search_result = create_test_search_result("Lightning Bolt", 5);
        let card_name = "lightning bolt".to_string();

        cache
            .insert(card_name.clone(), search_result.clone())
            .unwrap();
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(&card_name);
        assert_eq!(retrieved, Some(search_result));

        // Clean up
        cache.clear().unwrap();
        if temp_file.exists() {
            std::fs::remove_file(temp_file).ok();
        }
    }

    #[test]
    fn test_search_cache_entry_limit() {
        let temp_file = env::temp_dir().join(format!(
            "magic-proxy-limit-test-{}.json",
            std::process::id()
        ));

        let mut cache = create_search_cache_with_config(temp_file.clone(), 2).unwrap(); // Very small limit

        // Fill cache to limit
        let result1 = create_test_search_result("Card 1", 3);
        let result2 = create_test_search_result("Card 2", 3);
        let result3 = create_test_search_result("Card 3", 3);

        cache.insert("card1".to_string(), result1.clone()).unwrap();
        cache.insert("card2".to_string(), result2.clone()).unwrap();
        assert_eq!(cache.len(), 2);

        // Access card1 to make it more recently used
        cache.get(&"card1".to_string());

        // Insert card3 - should evict card2 (least recently used)
        cache.insert("card3".to_string(), result3.clone()).unwrap();
        assert_eq!(cache.len(), 2);
        assert!(cache.contains(&"card1".to_string()));
        assert!(!cache.contains(&"card2".to_string()));
        assert!(cache.contains(&"card3".to_string()));

        // Clean up
        cache.clear().unwrap();
        if temp_file.exists() {
            std::fs::remove_file(temp_file).ok();
        }
    }

    #[test]
    fn test_search_cache_persistence() {
        let temp_file = env::temp_dir().join(format!(
            "magic-proxy-persist-test-{}.json",
            std::process::id()
        ));

        let search_result = create_test_search_result("Lightning Bolt", 3);
        let card_name = "lightning bolt".to_string();

        // Create cache and add entry
        {
            let mut cache = create_search_cache_with_config(temp_file.clone(), 100).unwrap();
            cache
                .insert(card_name.clone(), search_result.clone())
                .unwrap();
            cache.save_to_storage().unwrap(); // Explicitly save
        }

        // Create new cache instance - should load from disk
        {
            let mut cache = create_search_cache_with_config(temp_file.clone(), 100).unwrap();
            assert_eq!(cache.len(), 1);
            assert!(cache.contains(&card_name));

            let retrieved = cache.get(&card_name);
            assert_eq!(retrieved, Some(search_result));

            // Clean up
            cache.clear().unwrap();
        }

        if temp_file.exists() {
            std::fs::remove_file(temp_file).ok();
        }
    }
}
