//! JSON-based storage strategy for search results
//!
//! This is a concrete implementation for storing CardSearchResult cache data
//! in a single JSON file, avoiding complex generic serialization issues.

use super::lru_cache::{CacheEntry, StorageStrategy};
use crate::error::ProxyError;
use crate::scryfall::CardSearchResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use time::OffsetDateTime;
use tracing::{debug, info};

/// JSON file format for storing search results cache data
#[derive(Debug, Serialize, Deserialize)]
struct SearchCacheData {
    pub entries: HashMap<String, CacheEntry<CardSearchResult>>,
    pub last_updated: OffsetDateTime,
    pub metadata: SearchCacheMetadata,
}

/// Metadata stored in search results cache file
#[derive(Debug, Serialize, Deserialize)]
struct SearchCacheMetadata {
    pub version: u32,
    pub cache_type: String,
    pub created_at: OffsetDateTime,
}

/// JSON-based storage strategy specifically for search results
pub struct SearchJsonStorage {
    cache_file: PathBuf,
    size_estimate: u64,
}

impl SearchJsonStorage {
    /// Create a new search results JSON storage strategy
    ///
    /// # Arguments
    /// * `cache_file` - Path to the JSON cache file
    /// * `size_estimate` - Estimated size per entry for quick calculations
    pub fn new(cache_file: PathBuf, size_estimate: u64) -> Result<Self, ProxyError> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = cache_file.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(ProxyError::Io)?;
                info!(cache_dir = %parent.display(), "Created search cache directory");
            }
        }

        Ok(Self {
            cache_file,
            size_estimate,
        })
    }
}

impl StorageStrategy<String, CardSearchResult> for SearchJsonStorage {
    fn load(&self) -> Result<HashMap<String, CacheEntry<CardSearchResult>>, ProxyError> {
        if !self.cache_file.exists() {
            debug!(
                cache_file = %self.cache_file.display(),
                "No existing search results cache found"
            );
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&self.cache_file).map_err(ProxyError::Io)?;

        // Try to parse as the current format
        let cache_data: SearchCacheData =
            serde_json::from_str(&content).map_err(ProxyError::Json)?;

        info!(
            entries = cache_data.entries.len(),
            cache_file = %self.cache_file.display(),
            "Loaded search results cache from disk"
        );

        Ok(cache_data.entries)
    }

    fn save(
        &self,
        entries: &HashMap<String, CacheEntry<CardSearchResult>>,
    ) -> Result<(), ProxyError> {
        let cache_data = SearchCacheData {
            entries: entries.clone(),
            last_updated: OffsetDateTime::now_utc(),
            metadata: SearchCacheMetadata {
                version: 1,
                cache_type: "SearchResults".to_string(),
                created_at: OffsetDateTime::now_utc(),
            },
        };

        let json = serde_json::to_string_pretty(&cache_data).map_err(ProxyError::Json)?;
        fs::write(&self.cache_file, json).map_err(ProxyError::Io)?;

        debug!(
            entries = entries.len(),
            cache_file = %self.cache_file.display(),
            "Saved search results cache to disk"
        );

        Ok(())
    }

    fn estimate_size(&self, _key: &String, _value: &CardSearchResult) -> u64 {
        // Use the provided size estimate
        // For more precision, we could serialize and measure, but that's expensive
        self.size_estimate
    }

    fn get_size_estimate(&self) -> u64 {
        self.size_estimate
    }

    fn evict_entry(&self, _key: &String, _value: &CardSearchResult) -> Result<(), ProxyError> {
        // For JSON storage, eviction just means removal from the in-memory HashMap
        // The actual file will be updated on the next save()
        // No additional cleanup needed
        Ok(())
    }

    fn strategy_name(&self) -> &'static str {
        "SearchJsonStorage"
    }
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

    fn create_test_storage() -> SearchJsonStorage {
        let temp_file = env::temp_dir().join(format!(
            "magic-proxy-search-test-{}.json",
            std::process::id()
        ));

        SearchJsonStorage::new(temp_file, 1024).unwrap()
    }

    #[test]
    #[ignore] // File system dependent test - see CLAUDE.md testing requirements
    fn test_search_json_storage_basic() {
        let storage = create_test_storage();

        // Test with empty cache
        let loaded = storage.load().unwrap();
        assert!(loaded.is_empty());

        // Create some test data
        let mut entries = HashMap::new();
        let test_data = create_test_search_result("Lightning Bolt", 5);
        let cache_entry = CacheEntry::new(test_data.clone());
        entries.insert("lightning bolt".to_string(), cache_entry);

        // Save and reload
        storage.save(&entries).unwrap();
        let reloaded = storage.load().unwrap();

        assert_eq!(reloaded.len(), 1);
        assert!(reloaded.contains_key("lightning bolt"));
        assert_eq!(reloaded["lightning bolt"].value.cards.len(), 5);

        // Clean up
        if storage.cache_file.exists() {
            fs::remove_file(&storage.cache_file).ok();
        }
    }

    #[test]
    fn test_search_json_size_estimation() {
        let storage = create_test_storage();
        let test_data = create_test_search_result("Test Card", 3);

        let size = storage.estimate_size(&"test".to_string(), &test_data);
        assert_eq!(size, 1024); // Should be the provided estimate
    }

    #[test]
    fn test_search_json_eviction() {
        let storage = create_test_storage();
        let test_data = create_test_search_result("Test Card", 2);

        // Eviction should not fail (it's a no-op for JSON storage)
        storage
            .evict_entry(&"test".to_string(), &test_data)
            .unwrap();
    }
}
