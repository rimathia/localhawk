use super::{client::ScryfallClient, models::*};
use crate::error::ProxyError;
use log::info;

const SCRYFALL_CARD_NAMES: &str = "https://api.scryfall.com/catalog/card-names";

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

    pub async fn search_card(&self, name: &str) -> Result<CardSearchResult, ProxyError> {
        let encoded_name = encode_card_name(name);
        let uri = format!(
            "https://api.scryfall.com/cards/search?q=name=\"{}\"&unique=prints",
            encoded_name
        );
        
        let response = self.call(&uri).await?;
        
        match response.json::<ScryfallSearchAnswer>().await {
            Ok(answer) => {
                let mut cards = Vec::new();
                for card_data in answer.data {
                    match Card::from_scryfall_object(&card_data) {
                        Ok(card) => cards.push(card),
                        Err(e) => {
                            info!("Skipping invalid card: {}", e);
                            continue;
                        }
                    }
                }
                
                Ok(CardSearchResult {
                    total_found: answer.total_cards as usize,
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
    name.replace(' ', "+").replace("//", "")
}