use crate::error::ProxyError;
use crate::scryfall::{ScryfallClient, ScryfallCardNames};
use crate::scryfall::client::ApiCallType;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

const CACHE_DURATION_DAYS: i64 = 1;

#[derive(Serialize, Deserialize, Debug)]
struct CachedCardNames {
    data: ScryfallCardNames,
    cached_at: OffsetDateTime,
}

pub struct CardNameCache {
    cache_file_path: PathBuf,
}

impl CardNameCache {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| ProxyError::Cache(format!("Failed to create cache directory: {}", e)))?;
        
        let cache_file_path = cache_dir.join("card_names.json");
        
        Ok(CardNameCache { cache_file_path })
    }

    fn get_cache_dir() -> Result<PathBuf, ProxyError> {
        ProjectDirs::from("", "", "magic-proxy")
            .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
            .ok_or_else(|| ProxyError::Cache("Could not determine cache directory".to_string()))
    }

    pub async fn get_card_names(&self, client: &ScryfallClient, force_update: bool) -> Result<ScryfallCardNames, ProxyError> {
        // Try to load from cache first (unless force update is requested)
        if !force_update {
            if let Ok(cached) = self.load_from_cache() {
                if self.is_cache_valid(&cached) {
                    log::info!("CACHE HIT: Using cached card names from {}", cached.cached_at);
                    ScryfallClient::record_cache_operation("https://api.scryfall.com/catalog/card-names", ApiCallType::CacheHit);
                    return Ok(cached.data);
                }
            }
        }

        // Cache miss or expired - fetch from API
        log::info!("CACHE MISS: Fetching fresh card names from Scryfall API");
        ScryfallClient::record_cache_operation("https://api.scryfall.com/catalog/card-names", ApiCallType::CacheMiss);
        let card_names = client.get_card_names().await?;
        
        // Save to cache
        self.save_to_cache(&card_names)?;
        
        Ok(card_names)
    }

    fn load_from_cache(&self) -> Result<CachedCardNames, ProxyError> {
        let content = fs::read_to_string(&self.cache_file_path)
            .map_err(|e| ProxyError::Cache(format!("Failed to read cache file: {}", e)))?;
            
        serde_json::from_str(&content)
            .map_err(|e| ProxyError::Cache(format!("Failed to parse cache file: {}", e)))
    }

    fn save_to_cache(&self, card_names: &ScryfallCardNames) -> Result<(), ProxyError> {
        let cached = CachedCardNames {
            data: card_names.clone(),
            cached_at: OffsetDateTime::now_utc(),
        };

        let content = serde_json::to_string_pretty(&cached)
            .map_err(|e| ProxyError::Cache(format!("Failed to serialize cache data: {}", e)))?;

        fs::write(&self.cache_file_path, content)
            .map_err(|e| ProxyError::Cache(format!("Failed to write cache file: {}", e)))?;

        log::info!("Saved card names to cache: {}", self.cache_file_path.display());
        Ok(())
    }

    fn is_cache_valid(&self, cached: &CachedCardNames) -> bool {
        let age = OffsetDateTime::now_utc() - cached.cached_at;
        age < Duration::days(CACHE_DURATION_DAYS)
    }

    pub fn get_cache_info(&self) -> Option<(OffsetDateTime, usize)> {
        self.load_from_cache()
            .ok()
            .map(|cached| (cached.cached_at, cached.data.names.len()))
    }

    pub fn clear_cache(&self) -> Result<(), ProxyError> {
        if self.cache_file_path.exists() {
            fs::remove_file(&self.cache_file_path)
                .map_err(|e| ProxyError::Cache(format!("Failed to remove cache file: {}", e)))?;
        }
        Ok(())
    }

    pub fn get_cache_path(&self) -> &PathBuf {
        &self.cache_file_path
    }
}

impl Default for CardNameCache {
    fn default() -> Self {
        Self::new().expect("Failed to create CardNameCache")
    }
}