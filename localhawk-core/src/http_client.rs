//! HTTP client abstraction for iOS sync operations
//! This module provides sync HTTP operations using ureq with native iOS TLS

#[cfg(feature = "ios")]
use crate::{
    error::ProxyError,
    scryfall::models::{Card, CardSearchResult, ScryfallCardNames, ScryfallSearchAnswer, ScryfallSetCodes, ScryfallSetsResponse},
};
#[cfg(feature = "ios")]
use log::debug;
#[cfg(feature = "ios")]
use std::sync::Mutex;
#[cfg(feature = "ios")]
use std::time::{Duration, Instant};

#[cfg(feature = "ios")]
const USER_AGENT: &str = "localhawk-core/0.1";
#[cfg(feature = "ios")]
const ACCEPT: &str = "*/*";
#[cfg(feature = "ios")]
const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);

#[cfg(feature = "ios")]
lazy_static::lazy_static! {
    static ref LAST_SCRYFALL_CALL: Mutex<Instant> =
        Mutex::new(Instant::now() - SCRYFALL_COOLDOWN);
}

/// Trait for sync HTTP operations (iOS only)
#[cfg(feature = "ios")]
pub trait HttpClient: Send + Sync {
    fn get_card_names(&self) -> Result<ScryfallCardNames, ProxyError>;
    fn get_set_codes(&self) -> Result<ScryfallSetCodes, ProxyError>;
    fn search_card(&self, name: &str) -> Result<CardSearchResult, ProxyError>;
    fn get_image_bytes(&self, url: &str) -> Result<Vec<u8>, ProxyError>;
}

/// Sync HTTP client using ureq with native iOS TLS
#[cfg(feature = "ios")]
pub struct UreqHttpClient {
    agent: ureq::Agent,
}

#[cfg(feature = "ios")]
impl UreqHttpClient {
    pub fn new() -> Result<Self, ProxyError> {
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .build();

        Ok(UreqHttpClient { agent })
    }

    fn call_with_rate_limit(&self, url: &str) -> Result<ureq::Response, ProxyError> {
        // Apply rate limiting for Scryfall API calls
        if url.contains(".scryfall.io") {
            let next_call = {
                let mut l = LAST_SCRYFALL_CALL.lock().unwrap();
                *l += SCRYFALL_COOLDOWN;
                *l
            };

            let now = Instant::now();
            if next_call > now {
                let sleep_duration = next_call - now;
                debug!("Rate limiting: sleeping for {:?}", sleep_duration);
                std::thread::sleep(sleep_duration);
            }

            *LAST_SCRYFALL_CALL.lock().unwrap() = Instant::now();
        }

        let response = self.agent
            .get(url)
            .set("Accept", ACCEPT)
            .call()
            .map_err(|e| ProxyError::from(e))?;

        Ok(response)
    }

    /// Helper function to search for meld result cards without recursive meld resolution
    /// Prioritizes meld results from the same set as the original card
    fn search_meld_result(&self, meld_result_name: &str, original_card_set: &str) -> Result<crate::scryfall::models::Card, ProxyError> {
        // Simple URL encoding - just replace spaces
        let encoded_name = meld_result_name.replace(" ", "+");
        let uri = format!(
            "https://api.scryfall.com/cards/search?q=name:\"{}\"&unique=prints",
            encoded_name
        );
        log::debug!("Searching for meld result with URI: {}", uri);

        let response = self.call_with_rate_limit(&uri)?;
        let answer: crate::scryfall::models::ScryfallSearchAnswer = response
            .into_json()
            .map_err(|e| ProxyError::Serialization(format!("Failed to parse meld search results: {}", e)))?;

        if answer.data.is_empty() {
            return Err(ProxyError::InvalidCard(format!(
                "Meld result '{}' not found",
                meld_result_name
            )));
        }

        // Debug: Log all search results for the meld result
        log::debug!(
            "Meld search for '{}' returned {} cards:",
            meld_result_name,
            answer.data.len()
        );

        // Convert all results to Cards and find the best match
        let mut all_meld_cards = Vec::new();
        for (i, card_data) in answer.data.iter().enumerate() {
            match crate::scryfall::models::Card::from_scryfall_object(&card_data) {
                Ok(card) => {
                    log::debug!(
                        "  [{}] '{}' (set: {}) - URL: {}",
                        i,
                        card.name,
                        card.set,
                        card.border_crop
                    );
                    all_meld_cards.push(card);
                }
                Err(e) => {
                    log::debug!("Failed to parse meld result card: {}", e);
                    continue;
                }
            }
        }

        if all_meld_cards.is_empty() {
            return Err(ProxyError::InvalidCard("No valid meld result card found".to_string()));
        }

        // Find a meld result card that matches the same set as the original card, or use the first one
        // This matches the logic in scryfall/api.rs
        let meld_card = all_meld_cards
            .iter()
            .find(|meld_card| meld_card.set == original_card_set)
            .or_else(|| all_meld_cards.first())
            .ok_or_else(|| {
                ProxyError::InvalidCard("No meld result card available".to_string())
            })?;

        log::debug!(
            "Selected meld result '{}' (set: {}) for original card set '{}'",
            meld_card.name,
            meld_card.set,
            original_card_set
        );

        Ok(meld_card.clone())
    }
}

#[cfg(feature = "ios")]
impl HttpClient for UreqHttpClient {
    fn get_card_names(&self) -> Result<ScryfallCardNames, ProxyError> {
        let response = self.call_with_rate_limit("https://api.scryfall.com/catalog/card-names")?;
        let mut card_names: ScryfallCardNames = response
            .into_json()
            .map_err(|e| ProxyError::Serialization(format!("Failed to parse card names: {}", e)))?;

        // Simple post processing - just sort card names
        card_names.names.sort();
        Ok(card_names)
    }

    fn get_set_codes(&self) -> Result<ScryfallSetCodes, ProxyError> {
        let response = self.call_with_rate_limit("https://api.scryfall.com/sets")?;
        let sets_response: ScryfallSetsResponse = response
            .into_json()
            .map_err(|e| ProxyError::Serialization(format!("Failed to parse sets: {}", e)))?;

        let codes = sets_response
            .data
            .into_iter()
            .map(|set| set.code.to_lowercase())
            .collect();

        let mut set_codes = ScryfallSetCodes {
            date: Some(time::OffsetDateTime::now_utc()),
            codes,
        };

        // Simple post processing - just sort set codes
        set_codes.codes.sort();
        Ok(set_codes)
    }

    fn search_card(&self, name: &str) -> Result<CardSearchResult, ProxyError> {
        // Simple URL encoding - just replace spaces
        let encoded_name = name.replace(" ", "+");
        let uri = format!(
            "https://api.scryfall.com/cards/search?q=name:\"{}\"&unique=prints",
            encoded_name
        );

        debug!("Searching Scryfall with URI: {}", uri);
        let response = self.call_with_rate_limit(&uri)?;

        let answer: ScryfallSearchAnswer = response
            .into_json()
            .map_err(|e| ProxyError::Serialization(format!("Failed to parse search results: {}", e)))?;

        let mut cards = Vec::new();

        for card_data in answer.data {
            match Card::from_scryfall_object(&card_data) {
                Ok(card) => cards.push(card),
                Err(e) => {
                    debug!("Skipping card due to conversion error: {}", e);
                    continue;
                }
            }
        }

        // Filter to exact name matches using shared business logic
        // Simple filtering - just take all cards for now
        let filtered_cards = cards;

        // Apply meld processing (shared logic from scryfall/api.rs search_card implementation)
        let mut processed_cards = filtered_cards;

        // Process meld cards - resolve empty meld_result_image_url fields
        for card in &mut processed_cards {
            if let Some(crate::scryfall::models::BackSide::ContributesToMeld {
                meld_result_name,
                meld_result_image_url,
                ..
            }) = &mut card.back_side
            {
                if meld_result_image_url.is_empty() {
                    log::debug!(
                        "Resolving meld result '{}' for card '{}'",
                        meld_result_name,
                        card.name
                    );

                    // Search for the meld result card, prioritizing same set
                    match self.search_meld_result(&meld_result_name, &card.set) {
                        Ok(meld_card) => {
                            log::debug!(
                                "Found meld result '{}' (set: {}) for card '{}' (set: {})",
                                meld_card.name,
                                meld_card.set,
                                card.name,
                                card.set
                            );
                            *meld_result_image_url = meld_card.border_crop.clone();
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to resolve meld result '{}' for card '{}': {}",
                                meld_result_name,
                                card.name,
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(CardSearchResult {
            cards: processed_cards.clone(),
            total_found: processed_cards.len(),
        })
    }

    fn get_image_bytes(&self, url: &str) -> Result<Vec<u8>, ProxyError> {
        let response = self.call_with_rate_limit(url)?;
        
        let mut bytes = Vec::new();
        std::io::copy(&mut response.into_reader(), &mut bytes)
            .map_err(|e| ProxyError::Io(e))?;
        
        Ok(bytes)
    }
}