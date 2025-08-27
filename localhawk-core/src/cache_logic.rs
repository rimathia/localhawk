//! Pure business logic for cache processing
//! 
//! This module contains all the pure business logic that doesn't depend on I/O operations.
//! It can be used by both async (desktop) and sync (iOS) implementations.

use crate::{
    error::ProxyError,
    lookup::CardNameLookup,
    scryfall::models::{ScryfallCardNames, ScryfallSetCodes},
};
use std::collections::HashSet;
use time::OffsetDateTime;
use tracing::{debug, info};

/// Pure business logic for processing card names into a fuzzy matching index
pub fn process_card_names_into_lookup(card_names: &ScryfallCardNames) -> CardNameLookup {
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

    lookup
}

/// Pure business logic for processing set codes into a HashSet for fast lookups
pub fn process_set_codes_into_hashset(set_codes: &ScryfallSetCodes) -> HashSet<String> {
    info!(
        set_code_count = set_codes.codes.len(),
        "Converting set codes into HashSet for fast lookups"
    );

    let codes_set: HashSet<String> = set_codes.codes.iter().cloned().collect();
    
    info!("Set codes processing complete");
    codes_set
}

/// Check if cached data is expired based on age threshold
pub fn is_cache_expired(cached_at: OffsetDateTime, max_age_hours: i64) -> bool {
    let age = OffsetDateTime::now_utc() - cached_at;
    age.whole_hours() > max_age_hours
}

/// Log cache hit information
pub fn log_cache_hit(cached_at: OffsetDateTime, item_count: usize, cache_type: &str) {
    let age = OffsetDateTime::now_utc() - cached_at;
    info!(
        age_hours = age.whole_hours(),
        item_count = item_count,
        cache_type = cache_type,
        "Using cached data from disk"
    );
}

/// Log cache miss information  
pub fn log_cache_miss(reason: &str, cache_type: &str) {
    info!(
        reason = reason,
        cache_type = cache_type,
        "Cache miss, will fetch from API"
    );
}

/// Default cache expiry time for card names (in hours)
pub const CARD_NAMES_CACHE_HOURS: i64 = 24;

/// Default cache expiry time for set codes (in hours) 
pub const SET_CODES_CACHE_HOURS: i64 = 24;