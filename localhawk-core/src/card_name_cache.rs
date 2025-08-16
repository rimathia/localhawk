use crate::error::ProxyError;
use crate::scryfall::{ScryfallCardNames, ScryfallClient};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};
use tracing::{debug, info, warn};

const CACHE_DURATION_DAYS: i64 = 1;

#[derive(Serialize, Deserialize, Debug)]
struct CachedCardNames {
    data: ScryfallCardNames,
    cached_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct CardNameCache {
    cache_file_path: PathBuf,
}

impl CardNameCache {
    pub fn new() -> Result<Self, ProxyError> {
        let cache_file_path = PathBuf::from(crate::get_card_names_cache_path());

        // Ensure the parent directory exists
        if let Some(parent_dir) = cache_file_path.parent() {
            fs::create_dir_all(parent_dir).map_err(|e| {
                ProxyError::Cache(format!("Failed to create cache directory: {}", e))
            })?;
        }

        Ok(CardNameCache { cache_file_path })
    }

    pub async fn get_card_names(
        &self,
        client: &ScryfallClient,
        force_update: bool,
    ) -> Result<ScryfallCardNames, ProxyError> {
        // Try to load from cache first (unless force update is requested)
        if !force_update {
            debug!(cache_file = %self.cache_file_path.display(), "Checking disk cache");

            if let Ok(cached) = self.load_from_cache() {
                let age = OffsetDateTime::now_utc() - cached.cached_at;
                info!(
                    age_hours = age.whole_hours(),
                    card_count = cached.data.names.len(),
                    "Loaded card names from disk cache"
                );

                if self.is_cache_valid(&cached) {
                    info!("Disk cache is valid, using cached data");
                    return Ok(cached.data);
                } else {
                    warn!(max_age_days = CACHE_DURATION_DAYS, "Disk cache expired");
                }
            } else {
                info!("No valid disk cache found");
            }
        } else {
            info!("Force update requested, skipping disk cache");
        }

        // Cache miss or expired - fetch from API
        info!("Fetching fresh card names from Scryfall API");
        let card_names = client.get_card_names().await?;

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
            data: card_names.clone(),
            cached_at: OffsetDateTime::now_utc(),
        };

        let content = serde_json::to_string_pretty(&cached)
            .map_err(|e| ProxyError::Cache(format!("Failed to serialize cache data: {}", e)))?;

        fs::write(&self.cache_file_path, content)
            .map_err(|e| ProxyError::Cache(format!("Failed to write cache file: {}", e)))?;

        log::info!(
            "Saved card names to cache: {}",
            self.cache_file_path.display()
        );
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

    pub fn save_current_data_to_disk(
        &self,
        card_names: &ScryfallCardNames,
    ) -> Result<(), ProxyError> {
        self.save_to_cache(card_names)
    }
}

impl Default for CardNameCache {
    fn default() -> Self {
        Self::new().expect("Failed to create CardNameCache")
    }
}
