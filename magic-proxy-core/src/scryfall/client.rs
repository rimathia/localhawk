use lazy_static::lazy_static;
use log::debug;
use tokio::time::{Duration, Instant};
use crate::error::ProxyError;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;

// Headers required according to https://scryfall.com/docs/api/
const USER_AGENT: &str = "magic-proxy-core/0.1";
const ACCEPT: &str = "*/*";
const SCRYFALL_COOLDOWN: Duration = Duration::from_millis(100);
const MAX_API_HISTORY: usize = 100;

#[derive(Debug, Clone)]
pub struct ApiCall {
    pub url: String,
    pub timestamp: OffsetDateTime,
    pub status_code: u16,
    pub success: bool,
    pub call_type: ApiCallType,
}

#[derive(Debug, Clone)]
pub enum ApiCallType {
    NetworkRequest,
    CacheHit,
    CacheMiss,
}

// Use a blocking mutex since we are only holding the lock to find out when we can call
lazy_static! {
    static ref LAST_SCRYFALL_CALL: std::sync::Mutex<Instant> =
        std::sync::Mutex::new(Instant::now() - SCRYFALL_COOLDOWN);
    static ref API_CALL_HISTORY: Arc<Mutex<Vec<ApiCall>>> = Arc::new(Mutex::new(Vec::new()));
}

#[derive(Debug)]
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
        
        let timestamp = OffsetDateTime::now_utc();
        match self.client.get(uri).send().await {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let success = response.status().is_success();
                
                // Record the API call
                let api_call = ApiCall {
                    url: uri.to_string(),
                    timestamp,
                    status_code,
                    success,
                    call_type: ApiCallType::NetworkRequest,
                };
                
                if let Ok(mut history) = API_CALL_HISTORY.lock() {
                    history.push(api_call);
                    // Keep only the last 100 calls to prevent memory issues
                    // Use drain to efficiently remove old entries
                    if history.len() > MAX_API_HISTORY {
                        let excess = history.len() - MAX_API_HISTORY;
                        history.drain(0..excess);
                    }
                }
                
                Ok(response)
            }
            Err(e) => {
                // Record failed API call
                let api_call = ApiCall {
                    url: uri.to_string(),
                    timestamp,
                    status_code: 0, // Unknown status for network errors
                    success: false,
                    call_type: ApiCallType::NetworkRequest,
                };
                
                if let Ok(mut history) = API_CALL_HISTORY.lock() {
                    history.push(api_call);
                    // Keep only the last 100 calls to prevent memory issues
                    // Use drain to efficiently remove old entries
                    if history.len() > MAX_API_HISTORY {
                        let excess = history.len() - MAX_API_HISTORY;
                        history.drain(0..excess);
                    }
                }
                
                Err(ProxyError::Network(e))
            }
        }
    }

    pub async fn get_image(&self, url: &str) -> Result<printpdf::image_crate::DynamicImage, ProxyError> {
        let response = self.call(url).await?;
        let bytes = response.bytes().await?;
        
        printpdf::image_crate::load_from_memory(&bytes)
            .map_err(|e| ProxyError::Cache(format!("Failed to load image: {}", e)))
    }

    /// Get the API call history for debugging purposes
    pub fn get_api_call_history() -> Vec<ApiCall> {
        API_CALL_HISTORY.lock().unwrap_or_else(|_| panic!("Failed to lock API call history")).clone()
    }

    /// Clear the API call history
    pub fn clear_api_call_history() {
        if let Ok(mut history) = API_CALL_HISTORY.lock() {
            history.clear();
        }
    }

    /// Record a cache operation (hit or miss)
    pub fn record_cache_operation(url: &str, call_type: ApiCallType) {
        let api_call = ApiCall {
            url: url.to_string(),
            timestamp: OffsetDateTime::now_utc(),
            status_code: 200, // Cache operations are always "successful"
            success: true,
            call_type,
        };

        if let Ok(mut history) = API_CALL_HISTORY.lock() {
            history.push(api_call);
            // Keep only the last MAX_API_HISTORY calls to prevent memory issues
            if history.len() > MAX_API_HISTORY {
                let excess = history.len() - MAX_API_HISTORY;
                history.drain(0..excess);
            }
        }
    }
}

impl Default for ScryfallClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ScryfallClient")
    }
}