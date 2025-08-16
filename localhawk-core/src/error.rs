use std::fmt;

#[derive(Debug)]
pub enum ProxyError {
    Network(reqwest::Error),
    Json(serde_json::Error),
    Pdf(String),
    Cache(String),
    InvalidCard(String),
    Io(std::io::Error),
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProxyError::Network(e) => write!(f, "Network error: {}", e),
            ProxyError::Json(e) => write!(f, "JSON parsing error: {}", e),
            ProxyError::Pdf(e) => write!(f, "PDF generation error: {}", e),
            ProxyError::Cache(e) => write!(f, "Cache error: {}", e),
            ProxyError::InvalidCard(e) => write!(f, "Invalid card: {}", e),
            ProxyError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ProxyError {}

impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        ProxyError::Network(err)
    }
}

impl From<serde_json::Error> for ProxyError {
    fn from(err: serde_json::Error) -> Self {
        ProxyError::Json(err)
    }
}

impl From<std::io::Error> for ProxyError {
    fn from(err: std::io::Error) -> Self {
        ProxyError::Io(err)
    }
}
