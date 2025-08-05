use super::{client::ScryfallClient, models::*};
use crate::error::ProxyError;
use log::info;

const SCRYFALL_CARD_NAMES: &str = "https://api.scryfall.com/catalog/card-names";
const SCRYFALL_SETS: &str = "https://api.scryfall.com/sets";

impl ScryfallClient {
    pub async fn get_card_names(&self) -> Result<ScryfallCardNames, ProxyError> {
        let response = self.call(SCRYFALL_CARD_NAMES).await?;
        let mut card_names: ScryfallCardNames = response.json().await?;
        
        card_names.date = Some(time::OffsetDateTime::now_utc());
        for name in card_names.names.iter_mut() {
            *name = name.to_lowercase();
        }
        
        Ok(card_names)
    }

    pub async fn get_set_codes(&self) -> Result<ScryfallSetCodes, ProxyError> {
        let response = self.call(SCRYFALL_SETS).await?;
        let sets_response: ScryfallSetsResponse = response.json().await?;
        
        let codes = sets_response.data.into_iter()
            .map(|set| set.code.to_lowercase())
            .collect();
        
        Ok(ScryfallSetCodes {
            date: Some(time::OffsetDateTime::now_utc()),
            codes,
        })
    }

    pub async fn search_card(&self, name: &str) -> Result<CardSearchResult, ProxyError> {
        let encoded_name = encode_card_name(name);
        let uri = format!(
            "https://api.scryfall.com/cards/search?q=name:\"{}\"+OR+name=\"{}\"&unique=prints",
            encoded_name, encoded_name
        );
        
        log::debug!("Searching Scryfall with URI: {}", uri);
        let response = self.call(&uri).await?;
        
        match response.json::<ScryfallSearchAnswer>().await {
            Ok(answer) => {
                let mut cards = Vec::new();
                let search_name_lower = name.to_lowercase();
                
                for card_data in answer.data {
                    match Card::from_scryfall_object(&card_data) {
                        Ok(card) => {
                            // Filter results to only include cards that exactly match our search name
                            let card_name_lower = card.name.to_lowercase();
                            if card_name_lower == search_name_lower {
                                log::debug!("Adding exact match: '{}' ({})", card.name, card.set);
                                cards.push(card);
                            } else {
                                log::debug!("Skipping non-exact match: '{}' != '{}'", card.name, name);
                            }
                        }
                        Err(e) => {
                            info!("Skipping invalid card: {}", e);
                            continue;
                        }
                    }
                }
                
                log::debug!("Filtered {} cards from {} total results", cards.len(), answer.total_cards);
                
                Ok(CardSearchResult {
                    total_found: cards.len(), // Use filtered count, not original total
                    cards,
                })
            }
            Err(e) => {
                info!("Error deserializing Scryfall search: {}", e);
                Err(ProxyError::Network(e))
            }
        }
    }
}

fn encode_card_name(name: &str) -> String {
    // Proper URL encoding for card names
    // Handle spaces, slashes, and other special characters
    name.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            '/' => "%2F".to_string(),
            '"' => "%22".to_string(),
            '\'' => "%27".to_string(),
            '&' => "%26".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c.to_string(),
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}