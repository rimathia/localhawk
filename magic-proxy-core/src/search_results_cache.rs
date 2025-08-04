use crate::error::ProxyError;
use crate::scryfall::{CardSearchResult, ScryfallClient};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use time::OffsetDateTime;
use tracing::{debug, info};

const CACHE_FILENAME: &str = "search_results_cache.json";

#[derive(Serialize, Deserialize, Debug)]
struct SearchResultsCacheData {
    entries: HashMap<String, CachedSearchResult>,
    last_updated: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CachedSearchResult {
    pub card_name: String,
    pub search_results: CardSearchResult,
    pub cached_at: OffsetDateTime,
    pub last_accessed: OffsetDateTime,
}

#[derive(Debug)]
pub struct SearchResultsCache {
    cache_file_path: PathBuf,
    pub cache: HashMap<String, CachedSearchResult>,
}

impl SearchResultsCache {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| ProxyError::Cache(format!("Failed to create search cache directory: {}", e)))?;
        
        let cache_file_path = cache_dir.join(CACHE_FILENAME);
        let mut cache = SearchResultsCache {
            cache_file_path,
            cache: HashMap::new(),
        };
        
        cache.load_from_disk()?;
        Ok(cache)
    }

    fn get_cache_dir() -> Result<PathBuf, ProxyError> {
        ProjectDirs::from("", "", "magic-proxy")
            .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
            .ok_or_else(|| ProxyError::Cache("Could not determine cache directory".to_string()))
    }

    pub async fn get_or_fetch_search_results(
        &mut self,
        client: &ScryfallClient,
        card_name: &str,
    ) -> Result<CardSearchResult, ProxyError> {
        let normalized_name = card_name.to_lowercase();
        
        // Check if we have cached results
        if let Some(cached) = self.cache.get_mut(&normalized_name) {
            cached.last_accessed = OffsetDateTime::now_utc();
            debug!(card_name = %card_name, "Search results cache HIT");
            return Ok(cached.search_results.clone());
        }
        
        debug!(card_name = %card_name, "Search results cache MISS, fetching from API");
        
        // Fetch from API
        let search_results = client.search_card(card_name).await?;
        
        // Cache the results
        let cached_result = CachedSearchResult {
            card_name: normalized_name.clone(),
            search_results: search_results.clone(),
            cached_at: OffsetDateTime::now_utc(),
            last_accessed: OffsetDateTime::now_utc(),
        };
        
        self.cache.insert(normalized_name, cached_result);
        self.save_to_disk()?;
        
        debug!(
            card_name = %card_name,
            results_count = search_results.cards.len(),
            cache_dir = %self.cache_file_path.parent().unwrap_or_else(|| std::path::Path::new("")).display(),
            "Search results cached to disk"
        );
        
        Ok(search_results)
    }
    
    pub fn force_evict(&mut self, card_name: &str) -> Result<(), ProxyError> {
        let normalized_name = card_name.to_lowercase();
        if self.cache.remove(&normalized_name).is_some() {
            self.save_to_disk()?;
            debug!(card_name = %card_name, "Force evicted search results from cache");
        }
        Ok(())
    }
    
    pub fn clear_cache(&mut self) -> Result<(), ProxyError> {
        self.cache.clear();
        self.save_to_disk()?;
        info!("Cleared all cached search results");
        Ok(())
    }
    
    pub fn size(&self) -> usize {
        self.cache.len()
    }
    
    pub fn contains(&self, card_name: &str) -> bool {
        self.cache.contains_key(&card_name.to_lowercase())
    }
    
    fn load_from_disk(&mut self) -> Result<(), ProxyError> {
        if !self.cache_file_path.exists() {
            debug!(
                cache_file = %self.cache_file_path.display(),
                cache_dir = %self.cache_file_path.parent().unwrap_or_else(|| std::path::Path::new("")).display(),
                "No existing search results cache found"
            );
            return Ok(());
        }
        
        let content = fs::read_to_string(&self.cache_file_path)
            .map_err(|e| ProxyError::Io(e))?;
        
        let cache_data: SearchResultsCacheData = serde_json::from_str(&content)
            .map_err(|e| ProxyError::Json(e))?;
        
        self.cache = cache_data.entries;
        
        let total_cards: usize = self.cache.values()
            .map(|entry| entry.search_results.cards.len())
            .sum();
        
        info!(
            cached_searches = self.cache.len(),
            total_card_results = total_cards,
            cache_dir = %self.cache_file_path.parent().unwrap_or_else(|| std::path::Path::new("")).display(),
            "Loaded search results cache from disk"
        );
        
        Ok(())
    }
    
    pub fn save_to_disk(&self) -> Result<(), ProxyError> {
        let cache_data = SearchResultsCacheData {
            entries: self.cache.clone(),
            last_updated: OffsetDateTime::now_utc(),
        };
        
        let json = serde_json::to_string_pretty(&cache_data)
            .map_err(|e| ProxyError::Json(e))?;
        
        fs::write(&self.cache_file_path, json)
            .map_err(|e| ProxyError::Io(e))?;
        
        debug!(
            cache_file = %self.cache_file_path.display(),
            entries = self.cache.len(),
            cache_dir = %self.cache_file_path.parent().unwrap_or_else(|| std::path::Path::new("")).display(),
            "Saved search results cache to disk"
        );
        
        Ok(())
    }
    
    pub fn get_cache_info(&self) -> Option<(OffsetDateTime, usize)> {
        if self.cache.is_empty() {
            return None;
        }
        
        // Get the oldest cache entry timestamp
        let oldest_entry = self.cache.values()
            .min_by_key(|entry| entry.cached_at)?;
        
        Some((oldest_entry.cached_at, self.cache.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::{Card, CardSearchResult};
    use std::env;

    fn create_test_search_result(card_name: &str, count: usize) -> CardSearchResult {
        let cards = (0..count).map(|i| Card {
            name: card_name.to_lowercase(),
            set: format!("set{}", i),
            language: "en".to_string(),
            border_crop: format!("https://example.com/image{}.jpg", i),
            border_crop_back: None,
            meld_result: None,
        }).collect();
        
        CardSearchResult {
            cards,
            total_found: count,
        }
    }
    
    fn create_test_cache() -> Result<SearchResultsCache, ProxyError> {
        let temp_dir = env::temp_dir().join(format!("magic-proxy-search-test-{}", std::process::id()));
        let cache_file = temp_dir.join(CACHE_FILENAME);
        fs::create_dir_all(&temp_dir).unwrap();
        
        Ok(SearchResultsCache {
            cache_file_path: cache_file,
            cache: HashMap::new(),
        })
    }
    
    #[test]
    fn test_cache_basic_operations() {
        let mut cache = create_test_cache().unwrap();
        let card_name = "Lightning Bolt";
        let search_result = create_test_search_result(card_name, 5);
        
        // Initially not cached
        assert!(!cache.contains(card_name));
        assert_eq!(cache.size(), 0);
        
        // Add to cache
        let cached_result = CachedSearchResult {
            card_name: card_name.to_lowercase(),
            search_results: search_result.clone(),
            cached_at: OffsetDateTime::now_utc(),
            last_accessed: OffsetDateTime::now_utc(),
        };
        
        cache.cache.insert(card_name.to_lowercase(), cached_result);
        
        // Should be cached now
        assert!(cache.contains(card_name));
        assert_eq!(cache.size(), 1);
        
        // Clean up
        cache.clear_cache().unwrap();
        assert_eq!(cache.size(), 0);
    }
    
    #[test]
    fn test_cache_persistence() {
        let temp_dir = env::temp_dir().join(format!("magic-proxy-search-persist-{}", std::process::id()));
        let cache_file = temp_dir.join(CACHE_FILENAME);
        fs::create_dir_all(&temp_dir).unwrap();
        
        let card_name = "Lightning Bolt";
        let search_result = create_test_search_result(card_name, 3);
        
        // Create cache and add entry
        {
            let mut cache = SearchResultsCache {
                cache_file_path: cache_file.clone(),
                cache: HashMap::new(),
            };
            
            let cached_result = CachedSearchResult {
                card_name: card_name.to_lowercase(),
                search_results: search_result,
                cached_at: OffsetDateTime::now_utc(),
                last_accessed: OffsetDateTime::now_utc(),
            };
            
            cache.cache.insert(card_name.to_lowercase(), cached_result);
            cache.save_to_disk().unwrap();
            assert_eq!(cache.size(), 1);
        }
        
        // Create new cache instance - should load from disk
        {
            let mut cache = SearchResultsCache {
                cache_file_path: cache_file,
                cache: HashMap::new(),
            };
            
            cache.load_from_disk().unwrap();
            assert_eq!(cache.size(), 1);
            assert!(cache.contains(card_name));
            
            // Clean up
            cache.clear_cache().unwrap();
        }
    }
    
    #[test]
    fn test_force_evict() {
        let mut cache = create_test_cache().unwrap();
        let card_name = "Lightning Bolt";
        let search_result = create_test_search_result(card_name, 2);
        
        let cached_result = CachedSearchResult {
            card_name: card_name.to_lowercase(),
            search_results: search_result,
            cached_at: OffsetDateTime::now_utc(),
            last_accessed: OffsetDateTime::now_utc(),
        };
        
        cache.cache.insert(card_name.to_lowercase(), cached_result);
        assert!(cache.contains(card_name));
        
        cache.force_evict(card_name).unwrap();
        assert!(!cache.contains(card_name));
        assert_eq!(cache.size(), 0);
    }
}