use lazy_static::lazy_static;
use log::debug;
use tokio::time::{Duration, Instant};
use crate::error::ProxyError;

// Headers required according to https://scryfall.com/docs/api/
const USER_AGENT: &str = "magic-proxy-core/0.1";
const ACCEPT: &str = "*/*";
const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);

// Use a blocking mutex since we are only holding the lock to find out when we can call
lazy_static! {
    static ref LAST_SCRYFALL_CALL: std::sync::Mutex<Instant> =
        std::sync::Mutex::new(Instant::now() - SCRYFALL_COOLDOWN);
}

pub struct ScryfallClient {
    client: reqwest::Client,
}

impl ScryfallClient {
    pub fn new() -> Result<Self, ProxyError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(USER_AGENT),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static(ACCEPT),
        );
        
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
            
        Ok(ScryfallClient { client })
    }

    pub async fn call(&self, uri: &str) -> Result<reqwest::Response, ProxyError> {
        let next_call = {
            let mut l = *LAST_SCRYFALL_CALL.lock().unwrap();
            l += SCRYFALL_COOLDOWN;
            l
        };
        tokio::time::sleep_until(next_call).await;
        debug!("calling scryfall API: {}", uri);
        Ok(self.client.get(uri).send().await?)
    }

    pub async fn get_image(&self, url: &str) -> Result<printpdf::image_crate::DynamicImage, ProxyError> {
        let response = self.call(url).await?;
        let bytes = response.bytes().await?;
        
        printpdf::image_crate::load_from_memory(&bytes)
            .map_err(|e| ProxyError::Cache(format!("Failed to load image: {}", e)))
    }
}

impl Default for ScryfallClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ScryfallClient")
    }
}