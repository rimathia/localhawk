use std::sync::{Arc, RwLock, OnceLock};
use image::DynamicImage;
use crate::{CardNameLookup, ScryfallClient, ImageCache, CardNameCache, ProxyError, NameLookupResult};

// Global singletons - initialized once, shared everywhere
static SCRYFALL_CLIENT: OnceLock<ScryfallClient> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<ImageCache>>> = OnceLock::new();
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();

pub fn get_scryfall_client() -> &'static ScryfallClient {
    SCRYFALL_CLIENT.get_or_init(|| {
        ScryfallClient::new().expect("Failed to create ScryfallClient")
    })
}

pub fn get_image_cache() -> &'static Arc<RwLock<ImageCache>> {
    IMAGE_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(ImageCache::new()))
    })
}

pub fn get_card_lookup() -> &'static Arc<RwLock<Option<CardNameLookup>>> {
    CARD_LOOKUP.get_or_init(|| {
        Arc::new(RwLock::new(None))
    })
}

// Convenience functions
pub async fn ensure_card_lookup_initialized() -> Result<(), ProxyError> {
    let lookup_ref = get_card_lookup();
    let needs_init = {
        let lookup = lookup_ref.read().unwrap();
        lookup.is_none()
    };
    
    if needs_init {
        let client = get_scryfall_client();
        let cache = CardNameCache::new()?;
        let card_names = cache.get_card_names(client, false).await?;
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        
        let mut lookup_guard = lookup_ref.write().unwrap();
        *lookup_guard = Some(lookup);
    }
    
    Ok(())
}

pub async fn force_update_card_lookup() -> Result<(), ProxyError> {
    let client = get_scryfall_client();
    let cache = CardNameCache::new()?;
    let card_names = cache.get_card_names(client, true).await?;
    let lookup = CardNameLookup::from_card_names(&card_names.names);
    
    let lookup_ref = get_card_lookup();
    let mut lookup_guard = lookup_ref.write().unwrap();
    *lookup_guard = Some(lookup);
    
    Ok(())
}

pub fn find_card_name(name: &str) -> Option<NameLookupResult> {
    let lookup_ref = get_card_lookup();
    let lookup = lookup_ref.read().unwrap();
    lookup.as_ref()?.find(name)
}

pub async fn get_or_fetch_image(url: &str) -> Result<DynamicImage, ProxyError> {
    let cache = get_image_cache();
    let client = get_scryfall_client();
    
    // Use the cache's get_or_fetch method properly
    let mut cache_guard = cache.write().unwrap();
    cache_guard.get_or_fetch(url, client).await
}

pub fn get_card_name_cache_info() -> Option<(time::OffsetDateTime, usize)> {
    match CardNameCache::new() {
        Ok(cache) => cache.get_cache_info(),
        Err(_) => None,
    }
}