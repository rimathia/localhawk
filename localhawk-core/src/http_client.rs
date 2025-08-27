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

        // Apply meld processing (shared logic from original search_card implementation)
        let mut processed_cards = filtered_cards;
        // Meld processing would go here, but for now we just use the cards as-is
        // The meld_result field doesn't exist on Card, so we skip this processing

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