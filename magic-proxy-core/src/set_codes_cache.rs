use crate::error::ProxyError;
use crate::scryfall::{ScryfallClient, models::ScryfallSetCodes};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};
use tracing::{info, debug, warn};

const CACHE_DURATION_DAYS: i64 = 1; // Same as card names cache for consistency

#[derive(Serialize, Deserialize, Debug)]
struct CachedSetCodes {
    data: ScryfallSetCodes,
    cached_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct SetCodesCache {
    cache_file_path: PathBuf,
}

impl SetCodesCache {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| ProxyError::Cache(format!("Failed to create cache directory: {}", e)))?;
        
        let cache_file_path = cache_dir.join("set_codes.json");
        
        Ok(SetCodesCache { cache_file_path })
    }

    fn get_cache_dir() -> Result<PathBuf, ProxyError> {
        ProjectDirs::from("", "", "magic-proxy")
            .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
            .ok_or_else(|| ProxyError::Cache("Could not determine cache directory".to_string()))
    }

    pub async fn get_set_codes(&self, client: &ScryfallClient, force_update: bool) -> Result<ScryfallSetCodes, ProxyError> {
        // Try to load from cache first (unless force update is requested)
        if !force_update {
            debug!(cache_file = %self.cache_file_path.display(), "Checking set codes disk cache");
            
            if let Ok(cached) = self.load_from_cache() {
                let age = OffsetDateTime::now_utc() - cached.cached_at;
                info!(
                    age_hours = age.whole_hours(),
                    set_count = cached.data.codes.len(),
                    "Loaded set codes from disk cache"
                );
                
                if self.is_cache_valid(&cached) {
                    info!("Set codes disk cache is valid, using cached data");
                    return Ok(cached.data);
                } else {
                    warn!(max_age_days = CACHE_DURATION_DAYS, "Set codes disk cache expired");
                }
            } else {
                info!("No valid set codes disk cache found");
            }
        } else {
            info!("Force update requested, skipping set codes disk cache");
        }

        // Cache miss or expired - fetch from API
        info!("Fetching fresh set codes from Scryfall API");
        let set_codes = client.get_set_codes().await?;
        
        // Save to cache
        self.save_to_cache(&set_codes)?;
        info!(
            set_code_count = set_codes.codes.len(),
            cache_file = %self.cache_file_path.display(),
            "Saved fresh set codes to disk cache"
        );
        
        Ok(set_codes)
    }

    fn load_from_cache(&self) -> Result<CachedSetCodes, ProxyError> {
        if !self.cache_file_path.exists() {
            debug!("Set codes cache file does not exist: {}", self.cache_file_path.display());
            return Err(ProxyError::Cache("Set codes cache file not found".to_string()));
        }

        let file_size = std::fs::metadata(&self.cache_file_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        debug!(
            file_size_kb = file_size / 1024,
            "Reading set codes cache file from disk"
        );

        let content = fs::read_to_string(&self.cache_file_path)
            .map_err(|e| ProxyError::Cache(format!("Failed to read set codes cache file: {}", e)))?;
            
        let parsed = serde_json::from_str(&content)
            .map_err(|e| ProxyError::Cache(format!("Failed to parse set codes cache file: {}", e)))?;

        debug!("Successfully parsed set codes cache file");
        Ok(parsed)
    }

    fn save_to_cache(&self, set_codes: &ScryfallSetCodes) -> Result<(), ProxyError> {
        let cached = CachedSetCodes {
            data: set_codes.clone(),
            cached_at: OffsetDateTime::now_utc(),
        };

        let content = serde_json::to_string_pretty(&cached)
            .map_err(|e| ProxyError::Cache(format!("Failed to serialize set codes cache data: {}", e)))?;

        fs::write(&self.cache_file_path, content)
            .map_err(|e| ProxyError::Cache(format!("Failed to write set codes cache file: {}", e)))?;

        log::info!("Saved set codes to cache: {}", self.cache_file_path.display());
        Ok(())
    }

    fn is_cache_valid(&self, cached: &CachedSetCodes) -> bool {
        let age = OffsetDateTime::now_utc() - cached.cached_at;
        age < Duration::days(CACHE_DURATION_DAYS)
    }

    pub fn get_cache_info(&self) -> Option<(OffsetDateTime, usize)> {
        self.load_from_cache()
            .ok()
            .map(|cached| (cached.cached_at, cached.data.codes.len()))
    }

    pub fn clear_cache(&self) -> Result<(), ProxyError> {
        if self.cache_file_path.exists() {
            fs::remove_file(&self.cache_file_path)
                .map_err(|e| ProxyError::Cache(format!("Failed to remove set codes cache file: {}", e)))?;
        }
        Ok(())
    }

    pub fn get_cache_path(&self) -> &PathBuf {
        &self.cache_file_path
    }
    
    pub fn save_current_data_to_disk(&self, set_codes: &ScryfallSetCodes) -> Result<(), ProxyError> {
        self.save_to_cache(set_codes)
    }
}

impl Default for SetCodesCache {
    fn default() -> Self {
        Self::new().expect("Failed to create SetCodesCache")
    }
}