use crate::{
    CardNameCache, CardNameLookup, ImageCache, NameLookupResult, ProxyError, 
    ScryfallClient, SearchResultsCache,
};
use printpdf::image_crate::DynamicImage;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{info, debug};

// Global singletons - initialized once, shared everywhere
static SCRYFALL_CLIENT: OnceLock<ScryfallClient> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<ImageCache>>> = OnceLock::new();
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<Arc<RwLock<SearchResultsCache>>> = OnceLock::new();

pub fn get_scryfall_client() -> &'static ScryfallClient {
    SCRYFALL_CLIENT.get_or_init(|| ScryfallClient::new().expect("Failed to create ScryfallClient"))
}

pub fn get_image_cache() -> &'static Arc<RwLock<ImageCache>> {
    IMAGE_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            ImageCache::new().expect("Failed to initialize image cache")
        ))
    })
}

pub fn get_card_lookup() -> &'static Arc<RwLock<Option<CardNameLookup>>> {
    CARD_LOOKUP.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn get_search_results_cache() -> &'static Arc<RwLock<SearchResultsCache>> {
    SEARCH_RESULTS_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            SearchResultsCache::new().expect("Failed to initialize search results cache")
        ))
    })
}

// Eager initialization function - call at application startup
pub fn initialize_caches() -> Result<(), ProxyError> {
    // Initialize image cache (loads from disk)
    let _image_cache = get_image_cache();
    info!("Image cache initialized at startup");
    
    // Initialize search results cache (loads from disk)
    let _search_cache = get_search_results_cache();
    info!("Search results cache initialized at startup");
    
    Ok(())
}

// Convenience functions
pub async fn ensure_card_lookup_initialized() -> Result<(), ProxyError> {
    let lookup_ref = get_card_lookup();
    let needs_init = {
        let lookup = lookup_ref.read().unwrap();
        lookup.is_none()
    };

    if needs_init {
        info!("Initializing CardNameLookup from disk cache");
        let client = get_scryfall_client();
        let cache = CardNameCache::new()?;
        
        // This will log disk cache operations internally
        let card_names = cache.get_card_names(client, false).await?;
        
        info!(
            card_count = card_names.names.len(),
            "Building fuzzy matching index from card names"
        );
        let start = std::time::Instant::now();
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let duration = start.elapsed();
        
        info!(
            duration_ms = duration.as_millis(),
            "CardNameLookup fuzzy index construction complete"
        );

        let mut lookup_guard = lookup_ref.write().unwrap();
        *lookup_guard = Some(lookup);
    } else {
        debug!("CardNameLookup already initialized");
    }

    Ok(())
}

pub async fn force_update_card_lookup() -> Result<(), ProxyError> {
    info!("Force updating CardNameLookup from Scryfall API");
    let client = get_scryfall_client();
    let cache = CardNameCache::new()?;
    
    // This will log the forced API fetch internally
    let card_names = cache.get_card_names(client, true).await?;
    
    info!(
        card_count = card_names.names.len(),
        "Building new fuzzy matching index from fresh data"
    );
    let start = std::time::Instant::now();
    let lookup = CardNameLookup::from_card_names(&card_names.names);
    let duration = start.elapsed();
    
    info!(
        duration_ms = duration.as_millis(),
        "Force update: CardNameLookup fuzzy index construction complete"
    );

    let lookup_ref = get_card_lookup();
    let mut lookup_guard = lookup_ref.write().unwrap();
    *lookup_guard = Some(lookup);

    Ok(())
}

pub fn find_card_name(name: &str) -> Option<NameLookupResult> {
    let lookup_ref = get_card_lookup();
    let lookup = lookup_ref.read().unwrap();
    let result = lookup.as_ref()?.find(name);
    
    match &result {
        Some(found) => debug!(input = %name, found = %found.name, "Fuzzy match found"),
        None => debug!(input = %name, "No fuzzy match found"),
    }
    
    result
}

pub async fn get_or_fetch_image(url: &str) -> Result<DynamicImage, ProxyError> {
    let cache = get_image_cache();
    let client = get_scryfall_client();

    // Try to get from cache first (note: this needs mutable access for LRU tracking)
    let cached_image = {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.get(url).map(|x| x.clone())
    };
    
    match cached_image {
        Some(image) => {
            Ok(image)
        },
        None => {
            debug!(url = %url, "Image cache MISS, fetching from network");
            let new_image = client.get_image(url).await?;
            
            // Insert into cache (this handles disk persistence and LRU eviction)
            {
                let mut cache_guard = cache.write().unwrap();
                cache_guard.insert(url.to_string(), new_image.clone())?;
            }
            
            Ok(new_image)
        }
    }
}

pub fn get_card_name_cache_info() -> Option<(time::OffsetDateTime, usize)> {
    match CardNameCache::new() {
        Ok(cache) => cache.get_cache_info(),
        Err(_) => None,
    }
}

pub async fn get_or_fetch_search_results(card_name: &str) -> Result<crate::scryfall::CardSearchResult, ProxyError> {
    let client = get_scryfall_client();
    let cache = get_search_results_cache();
    
    // Check cache first (separate scope to release lock)
    let cached_result = {
        let mut cache_guard = cache.write().unwrap();
        if let Some(cached) = cache_guard.cache.get_mut(&card_name.to_lowercase()) {
            cached.last_accessed = time::OffsetDateTime::now_utc();
            debug!(card_name = %card_name, "Search results cache HIT");
            Some(cached.search_results.clone())
        } else {
            None
        }
    };
    
    if let Some(result) = cached_result {
        return Ok(result);
    }
    
    // Cache miss - fetch from API
    debug!(card_name = %card_name, "Search results cache MISS, fetching from API");
    let search_results = client.search_card(card_name).await?;
    
    // Insert into cache (separate scope to release lock)
    {
        let mut cache_guard = cache.write().unwrap();
        let cached_result = crate::search_results_cache::CachedSearchResult {
            card_name: card_name.to_lowercase(),
            search_results: search_results.clone(),
            cached_at: time::OffsetDateTime::now_utc(),
            last_accessed: time::OffsetDateTime::now_utc(),
        };
        
        cache_guard.cache.insert(card_name.to_lowercase(), cached_result);
        cache_guard.save_to_disk()?;
        
        debug!(
            card_name = %card_name,
            results_count = search_results.cards.len(),
            "Search results cached to disk"
        );
    }
    
    Ok(search_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_caches() {
        // Should not panic and should return Ok
        let result = initialize_caches();
        assert!(result.is_ok());
        
        // Cache should be initialized and accessible
        let cache = get_image_cache();
        let cache_guard = cache.read().unwrap();
        // Should not panic when accessing cache methods
        let _size = cache_guard.size();
    }
}
