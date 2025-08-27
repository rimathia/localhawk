//! iOS-specific sync cache implementations
//! 
//! This module provides sync versions of cache operations for iOS,
//! using the pure business logic from cache_logic.rs and sync I/O.

#[cfg(feature = "ios")]
use crate::{
    cache_logic::{
        process_card_names_into_lookup, process_set_codes_into_hashset,
        is_cache_expired, log_cache_hit, log_cache_miss,
        CARD_NAMES_CACHE_HOURS, SET_CODES_CACHE_HOURS
    },
    error::ProxyError,
    http_client::{HttpClient, UreqHttpClient},
    lookup::CardNameLookup,
    scryfall::models::{ScryfallCardNames, ScryfallSetCodes},
};
#[cfg(feature = "ios")]
use directories::ProjectDirs;
#[cfg(feature = "ios")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ios")]
use std::{collections::HashSet, fs, path::PathBuf};
#[cfg(feature = "ios")]
use time::OffsetDateTime;
#[cfg(feature = "ios")]
use tracing::{debug, info, error};

#[cfg(feature = "ios")]
#[derive(Serialize, Deserialize)]
struct CachedCardNames {
    cached_at: OffsetDateTime,
    data: ScryfallCardNames,
}

#[cfg(feature = "ios")]
#[derive(Serialize, Deserialize)]
struct CachedSetCodes {
    cached_at: OffsetDateTime,
    data: ScryfallSetCodes,
}

/// Sync iOS-specific card name cache
#[cfg(feature = "ios")]
pub struct CardNameCacheSync {
    cache_file_path: PathBuf,
}

#[cfg(feature = "ios")]
impl CardNameCacheSync {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_dir = ProjectDirs::from("", "", "localhawk")
            .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("localhawk-cache"));

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                ProxyError::Cache(format!("Failed to create cache directory: {}", e))
            })?;
        }

        let cache_file_path = cache_dir.join("card_names.json");

        Ok(CardNameCacheSync { cache_file_path })
    }

    pub fn get_card_names_sync(
        &self,
        client: &UreqHttpClient,
        force_update: bool,
    ) -> Result<ScryfallCardNames, ProxyError> {
        // Try to load from cache first (unless force update is requested)
        if !force_update {
            debug!(cache_file = %self.cache_file_path.display(), "Checking disk cache");

            if let Ok(cached) = self.load_from_cache() {
                if !is_cache_expired(cached.cached_at, CARD_NAMES_CACHE_HOURS) {
                    log_cache_hit(cached.cached_at, cached.data.names.len(), "card_names");
                    return Ok(cached.data);
                } else {
                    log_cache_miss("cache expired", "card_names");
                }
            } else {
                log_cache_miss("cache file not found or corrupted", "card_names");
            }
        } else {
            log_cache_miss("force update requested", "card_names");
        }

        // Fetch from API
        info!("Fetching fresh card names from Scryfall API");
        let card_names = client.get_card_names()?;

        // Save to cache
        self.save_to_cache(&card_names)?;
        info!(
            card_count = card_names.names.len(),
            cache_file = %self.cache_file_path.display(),
            "Saved fresh card names to disk cache"
        );

        Ok(card_names)
    }

    fn load_from_cache(&self) -> Result<CachedCardNames, ProxyError> {
        if !self.cache_file_path.exists() {
            debug!(
                "Cache file does not exist: {}",
                self.cache_file_path.display()
            );
            return Err(ProxyError::Cache("Cache file not found".to_string()));
        }

        let file_size = std::fs::metadata(&self.cache_file_path)
            .map(|m| m.len())
            .unwrap_or(0);

        debug!(
            file_size_kb = file_size / 1024,
            "Reading cache file from disk"
        );

        let content = fs::read_to_string(&self.cache_file_path)
            .map_err(|e| ProxyError::Cache(format!("Failed to read cache file: {}", e)))?;

        let parsed = serde_json::from_str(&content)
            .map_err(|e| ProxyError::Cache(format!("Failed to parse cache file: {}", e)))?;

        debug!("Successfully parsed cache file");
        Ok(parsed)
    }

    fn save_to_cache(&self, card_names: &ScryfallCardNames) -> Result<(), ProxyError> {
        let cached = CachedCardNames {
            cached_at: OffsetDateTime::now_utc(),
            data: card_names.clone(),
        };

        let json_content = serde_json::to_string_pretty(&cached)
            .map_err(|e| ProxyError::Cache(format!("Failed to serialize cache data: {}", e)))?;

        fs::write(&self.cache_file_path, json_content)
            .map_err(|e| ProxyError::Cache(format!("Failed to write cache file: {}", e)))?;

        debug!(
            cache_file = %self.cache_file_path.display(),
            "Saved card names cache to disk"
        );

        Ok(())
    }
}

/// Sync iOS-specific set codes cache  
#[cfg(feature = "ios")]
pub struct SetCodesCacheSync {
    cache_file_path: PathBuf,
}

#[cfg(feature = "ios")]
impl SetCodesCacheSync {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_dir = ProjectDirs::from("", "", "localhawk")
            .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("localhawk-cache"));

        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                ProxyError::Cache(format!("Failed to create cache directory: {}", e))
            })?;
        }

        let cache_file_path = cache_dir.join("set_codes.json");

        Ok(SetCodesCacheSync { cache_file_path })
    }

    pub fn get_set_codes_sync(
        &self,
        client: &UreqHttpClient,
        force_update: bool,
    ) -> Result<ScryfallSetCodes, ProxyError> {
        // Try to load from cache first (unless force update is requested)
        if !force_update {
            debug!(cache_file = %self.cache_file_path.display(), "Checking disk cache");

            if let Ok(cached) = self.load_from_cache() {
                if !is_cache_expired(cached.cached_at, SET_CODES_CACHE_HOURS) {
                    log_cache_hit(cached.cached_at, cached.data.codes.len(), "set_codes");
                    return Ok(cached.data);
                } else {
                    log_cache_miss("cache expired", "set_codes");
                }
            } else {
                log_cache_miss("cache file not found or corrupted", "set_codes");
            }
        } else {
            log_cache_miss("force update requested", "set_codes");
        }

        // Fetch from API
        info!("Fetching fresh set codes from Scryfall API");
        let set_codes = client.get_set_codes()?;

        // Save to cache
        self.save_to_cache(&set_codes)?;
        info!(
            set_count = set_codes.codes.len(),
            cache_file = %self.cache_file_path.display(),
            "Saved fresh set codes to disk cache"
        );

        Ok(set_codes)
    }

    fn load_from_cache(&self) -> Result<CachedSetCodes, ProxyError> {
        if !self.cache_file_path.exists() {
            debug!(
                "Cache file does not exist: {}",
                self.cache_file_path.display()
            );
            return Err(ProxyError::Cache("Cache file not found".to_string()));
        }

        let file_size = std::fs::metadata(&self.cache_file_path)
            .map(|m| m.len())
            .unwrap_or(0);

        debug!(
            file_size_kb = file_size / 1024,
            "Reading cache file from disk"
        );

        let content = fs::read_to_string(&self.cache_file_path)
            .map_err(|e| ProxyError::Cache(format!("Failed to read cache file: {}", e)))?;

        let parsed = serde_json::from_str(&content)
            .map_err(|e| ProxyError::Cache(format!("Failed to parse cache file: {}", e)))?;

        debug!("Successfully parsed cache file");
        Ok(parsed)
    }

    fn save_to_cache(&self, set_codes: &ScryfallSetCodes) -> Result<(), ProxyError> {
        let cached = CachedSetCodes {
            cached_at: OffsetDateTime::now_utc(),
            data: set_codes.clone(),
        };

        let json_content = serde_json::to_string_pretty(&cached)
            .map_err(|e| ProxyError::Cache(format!("Failed to serialize cache data: {}", e)))?;

        fs::write(&self.cache_file_path, json_content)
            .map_err(|e| ProxyError::Cache(format!("Failed to write cache file: {}", e)))?;

        debug!(
            cache_file = %self.cache_file_path.display(),
            "Saved set codes cache to disk"
        );

        Ok(())
    }
}

/// Initialize card name lookup using pure business logic
#[cfg(feature = "ios")]
pub fn initialize_card_lookup_sync(client: &UreqHttpClient) -> Result<(CardNameLookup, Option<(OffsetDateTime, usize)>), ProxyError> {
    let cache = CardNameCacheSync::new()?;
    let card_names = cache.get_card_names_sync(client, false)?;
    
    // Use pure business logic to process the data
    let lookup = process_card_names_into_lookup(&card_names);
    let cache_info = card_names.date.map(|date| (date, card_names.names.len()));
    
    Ok((lookup, cache_info))
}

/// Initialize set codes using pure business logic
#[cfg(feature = "ios")]
pub fn initialize_set_codes_sync(client: &UreqHttpClient) -> Result<HashSet<String>, ProxyError> {
    let cache = SetCodesCacheSync::new()?;
    let set_codes = cache.get_set_codes_sync(client, false)?;
    
    // Use pure business logic to process the data
    let codes_set = process_set_codes_into_hashset(&set_codes);
    
    Ok(codes_set)
}