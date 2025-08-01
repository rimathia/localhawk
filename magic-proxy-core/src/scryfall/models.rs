use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;
use crate::error::ProxyError;

#[derive(Serialize, Deserialize, Debug)]
pub struct ScryfallCardNames {
    pub object: String,
    pub uri: String,
    pub total_values: i32,
    pub date: Option<OffsetDateTime>,
    #[serde(alias = "data")]
    pub names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ScryfallSearchAnswer {
    pub object: String,
    pub total_cards: i32,
    pub has_more: bool,
    pub next_page: Option<String>,
    pub data: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Card {
    pub name: String,
    pub set: String,
    pub language: String,
    pub border_crop: String,
    pub border_crop_back: Option<String>,
    pub meld_result: Option<String>,
}

impl Card {
    pub fn from_scryfall_object(
        d: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Card, ProxyError> {
        let name = d["name"]
            .as_str()
            .ok_or_else(|| ProxyError::InvalidCard("Missing name field".to_string()))?
            .to_string()
            .to_lowercase();
            
        let set = d["set"]
            .as_str()
            .ok_or_else(|| ProxyError::InvalidCard("Missing set field".to_string()))?
            .to_string()
            .to_lowercase();
            
        let language = d["lang"]
            .as_str()
            .ok_or_else(|| ProxyError::InvalidCard("Missing lang field".to_string()))?
            .to_string()
            .to_lowercase();

        let (border_crop, border_crop_back) = {
            if d.contains_key("image_uris") {
                let front = d["image_uris"]["border_crop"]
                    .as_str()
                    .ok_or_else(|| ProxyError::InvalidCard("Missing border_crop image".to_string()))?
                    .to_string();
                (front, None)
            } else if d.contains_key("card_faces") {
                let card_faces = d["card_faces"]
                    .as_array()
                    .ok_or_else(|| ProxyError::InvalidCard("Invalid card_faces structure".to_string()))?;
                    
                if card_faces.len() != 2 {
                    return Err(ProxyError::InvalidCard("Expected 2 card faces".to_string()));
                }
                
                let front = card_faces[0]["image_uris"]["border_crop"]
                    .as_str()
                    .ok_or_else(|| ProxyError::InvalidCard("Missing front border_crop".to_string()))?
                    .to_string();
                    
                let back = card_faces[1]["image_uris"]["border_crop"]
                    .as_str()
                    .ok_or_else(|| ProxyError::InvalidCard("Missing back border_crop".to_string()))?
                    .to_string();
                    
                (front, Some(back))
            } else {
                return Err(ProxyError::InvalidCard("No image data found".to_string()));
            }
        };

        let meld_result = if d["layout"] == "meld" {
            let all_parts = d["all_parts"]
                .as_array()
                .ok_or_else(|| ProxyError::InvalidCard("Invalid all_parts for meld card".to_string()))?;
                
            let meld_result_name = all_parts
                .iter()
                .find(|entry| entry["component"] == "meld_result")
                .and_then(|entry| entry["name"].as_str())
                .ok_or_else(|| ProxyError::InvalidCard("Missing meld result".to_string()))?
                .to_lowercase();
                
            if meld_result_name != name {
                Some(meld_result_name)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Card {
            name,
            set,
            language,
            border_crop,
            border_crop_back,
            meld_result,
        })
    }
}

#[derive(Debug)]
pub struct CardSearchResult {
    pub cards: Vec<Card>,
    pub total_found: usize,
}

pub fn get_minimal_scryfall_languages() -> HashSet<String> {
    HashSet::from(
        [
            "en", "es", "fr", "de", "it", "pt", "ja", "ko", "ru", "zhs", "zht", "he", "la", "grc",
            "ar", "sa", "ph",
        ]
        .map(String::from),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meld_card_parsing() {
        // Test data from MagicHawk: Urza, Lord Protector meld card
        let urza_lord_protector = r#"{"object":"card","id":"8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","oracle_id":"df2af646-3e5b-43a3-8f3e-50565889f456","multiverse_ids":[588288],"mtgo_id":105072,"arena_id":82710,"tcgplayer_id":448412,"cardmarket_id":678194,"name":"Urza, Lord Protector","lang":"en","released_at":"2022-11-18","uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","scryfall_uri":"https://scryfall.com/card/bro/225/urza-lord-protector?utm_source=api","layout":"meld","highres_image":false,"image_status":"lowres","image_uris":{"small":"https://cards.scryfall.io/small/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","normal":"https://cards.scryfall.io/normal/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","large":"https://cards.scryfall.io/large/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","png":"https://cards.scryfall.io/png/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.png?1670539417","art_crop":"https://cards.scryfall.io/art_crop/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417","border_crop":"https://cards.scryfall.io/border_crop/front/8/a/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083.jpg?1670539417"},"mana_cost":"{1}{W}{U}","cmc":3.0,"type_line":"Legendary Creature — Human Artificer","oracle_text":"Artifact, instant, and sorcery spells you cast cost {1} less to cast.\n{7}: If you both own and control Urza, Lord Protector and an artifact named The Mightstone and Weakstone, exile them, then meld them into Urza, Planeswalker. Activate only as a sorcery.","power":"2","toughness":"4","colors":["U","W"],"color_identity":["U","W"],"keywords":["Meld"],"all_parts":[{"object":"related_card","id":"40a01679-3224-427e-bd1d-b797b0ab68b7","component":"meld_result","name":"Urza, Planeswalker","type_line":"Legendary Planeswalker — Urza","uri":"https://api.scryfall.com/cards/40a01679-3224-427e-bd1d-b797b0ab68b7"},{"object":"related_card","id":"02aea379-b444-46a3-82f4-3038f698d4f4","component":"meld_part","name":"The Mightstone and Weakstone","type_line":"Legendary Artifact — Powerstone","uri":"https://api.scryfall.com/cards/02aea379-b444-46a3-82f4-3038f698d4f4"},{"object":"related_card","id":"8aefe8bd-216a-4ec1-9362-3f9dbf7fd083","component":"meld_part","name":"Urza, Lord Protector","type_line":"Legendary Creature — Human Artificer","uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083"}],"legalities":{"standard":"legal","future":"legal","historic":"legal","gladiator":"legal","pioneer":"legal","explorer":"legal","modern":"legal","legacy":"legal","pauper":"not_legal","vintage":"legal","penny":"not_legal","commander":"legal","brawl":"legal","historicbrawl":"legal","alchemy":"legal","paupercommander":"not_legal","duel":"legal","oldschool":"not_legal","premodern":"not_legal"},"games":["paper","mtgo","arena"],"reserved":false,"foil":true,"nonfoil":true,"finishes":["nonfoil","foil"],"oversized":false,"promo":false,"reprint":false,"variation":false,"set_id":"4219a14e-6701-4ddd-a185-21dc054ab19b","set":"bro","set_name":"The Brothers' War","set_type":"expansion","set_uri":"https://api.scryfall.com/sets/4219a14e-6701-4ddd-a185-21dc054ab19b","set_search_uri":"https://api.scryfall.com/cards/search?order=set\u0026q=e%3Abro\u0026unique=prints","scryfall_set_uri":"https://scryfall.com/sets/bro?utm_source=api","rulings_uri":"https://api.scryfall.com/cards/8aefe8bd-216a-4ec1-9362-3f9dbf7fd083/rulings","prints_search_uri":"https://api.scryfall.com/cards/search?order=released\u0026q=oracleid%3Adf2af646-3e5b-43a3-8f3e-50565889f456\u0026unique=prints","collector_number":"225","digital":false,"rarity":"mythic","card_back_id":"58a4215b-9f3d-40d4-bc05-d8d3cc2354d9","artist":"Ryan Pancoast","artist_ids":["89cc9475-dda2-4d13-bf88-54b92867a25c"],"illustration_id":"c1abe983-d141-4884-9812-2593773f1a59","border_color":"black","frame":"2015","frame_effects":["legendary"],"security_stamp":"oval","full_art":false,"textless":false,"booster":true,"story_spotlight":false,"edhrec_rank":7316,"prices":{"usd":"26.65","usd_foil":"31.39","usd_etched":null,"eur":"19.24","eur_foil":"29.19","tix":"5.82"},"related_uris":{"gatherer":"https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=588288","tcgplayer_infinite_articles":"https://infinite.tcgplayer.com/search?contentMode=article\u0026game=magic\u0026partner=scryfall\u0026q=Urza%2C+Lord+Protector\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","tcgplayer_infinite_decks":"https://infinite.tcgplayer.com/search?contentMode=deck\u0026game=magic\u0026partner=scryfall\u0026q=Urza%2C+Lord+Protector\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","edhrec":"https://edhrec.com/route/?cc=Urza%2C+Lord+Protector"},"purchase_uris":{"tcgplayer":"https://www.tcgplayer.com/product/448412?page=1\u0026utm_campaign=affiliate\u0026utm_medium=api\u0026utm_source=scryfall","cardmarket":"https://www.cardmarket.com/en/Magic/Products/Search?referrer=scryfall\u0026searchString=Urza%2C+Lord+Protector\u0026utm_campaign=card_prices\u0026utm_medium=text\u0026utm_source=scryfall","cardhoarder":"https://www.cardhoarder.com/cards/105072?affiliate_id=scryfall\u0026ref=card-profile\u0026utm_campaign=affiliate\u0026utm_medium=card\u0026utm_source=scryfall"}}"#;
        
        let v: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(urza_lord_protector).unwrap();
        let card = Card::from_scryfall_object(&v).unwrap();
        
        assert_eq!(card.name, "urza, lord protector");
        assert_eq!(card.set, "bro");
        assert_eq!(card.language, "en");
        assert_eq!(
            card.meld_result,
            Some("urza, planeswalker".to_string())
        );
    }

    #[test]
    fn test_split_card_parsing() {
        // Test using actual file data - Consecrate // Consume is a split card
        let input = include_str!("../../test_data/card_data_consecrate.json");
        let list: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(input).unwrap();
        let card = Card::from_scryfall_object(&list[0]).unwrap();
        assert_eq!(card.name, "consecrate // consume");
        // Split cards use single image_uris, not card_faces with separate images
        assert!(card.border_crop_back.is_none());
        assert!(!card.border_crop.is_empty());
    }

    #[test]
    fn test_missing_image_data_error() {
        // Test card with missing image data should return error
        let input = include_str!("../../test_data/card_data_memory_lapse.json");
        let list: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(input).unwrap();
        let result = Card::from_scryfall_object(&list[0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_adventure_card_parsing() {
        let input = include_str!("../../test_data/card_data_illithid.json");
        let list: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(input).unwrap();
        let card = Card::from_scryfall_object(&list[0]).unwrap();
        assert_eq!(card.name, "illithid harvester // plant tadpoles");
    }

    #[test]
    fn test_flip_card_parsing() {
        let input = include_str!("../../test_data/card_data_erayo.json");
        let list: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(input).unwrap();
        let card = Card::from_scryfall_object(&list[0]).unwrap();
        assert_eq!(card.name, "erayo, soratami ascendant // erayo's essence");
    }

    #[test]
    fn test_supported_languages() {
        let languages = get_minimal_scryfall_languages();
        assert!(languages.contains("en"));
        assert!(languages.contains("ja"));
        assert!(languages.contains("fr"));
        assert!(languages.contains("de"));
        assert_eq!(languages.len(), 17);
    }
}