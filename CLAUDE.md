# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Magic Card Proxy Sheet Generator - A Rust GUI application that creates PDF proxy sheets for Magic: The Gathering cards. Users specify a list of cards and the application generates a printable PDF with card images arranged in a grid layout.

## Key Dependencies

- **iced** - Cross-platform GUI framework (Elm-inspired architecture)
- **reqwest** - HTTP client for Scryfall API calls and image downloads (with `blocking`, `json`, `rustls-tls` features)
- **printpdf** - PDF generation library (with `embedded_images` feature)
- **serde** - JSON serialization for API responses (with `derive` feature)
- **tokio** - Async runtime for concurrent operations (with `time` feature)
- **time** - Date/time handling for cache management (with `serde`, `formatting` features)
- **lazy_static** - For global state management (rate limiting)

## Development Commands

### Build and Run
- `cargo build` - Compile the project
- `cargo run` - Build and run the application
- `cargo check` - Check for compilation errors without building

### Testing and Quality
- `cargo test` - Run all tests
- `cargo clippy` - Run Rust linter
- `cargo fmt` - Format code according to Rust standards

### Development Workflow
- `cargo clean` - Remove build artifacts from target directory
- `cargo update` - Update dependencies in Cargo.lock

## Architecture

### Application Structure (Iced Pattern)
- **State**: CardList, DownloadProgress, PdfSettings
- **Messages**: AddCard, RemoveCard, SearchCard, GeneratePdf, DownloadComplete
- **Update Logic**: Handle user actions, API responses, and state transitions
- **View Logic**: Render UI components and handle user interactions

### Data Flow
1. User inputs card names → Scryfall API search
2. Card selection → Add to generation list
3. Image download and caching (async)
4. PDF layout and generation
5. File save dialog

### External APIs
- **Scryfall API**: Magic card database and image source
  - Rate limit: 10 requests/second (100ms delay between requests)
  - Required headers: `User-Agent` and `Accept`
  - Search endpoint: `https://api.scryfall.com/cards/search?q=name=!{CARD_NAME}&unique=prints`
  - Card names catalog: `https://api.scryfall.com/catalog/card-names`
  - Image field: `border_crop` from `image_uris` or `card_faces[].image_uris`

## Implementation Patterns from MagicHawk

### Rate Limiting
- Use `lazy_static` Mutex to track last API call timestamp
- Enforce 100ms cooldown between Scryfall API requests
- Both async and blocking variants available

### Scryfall Client Structure
```rust
pub struct ScryfallClient {
    client: reqwest::Client,
}
// Pre-configured with required headers: User-Agent and Accept
```

### Card Data Model
```rust
pub struct MinimalScryfallObject {
    pub name: String,
    pub set: String, 
    pub language: String,
    pub border_crop: String,           // Front image URL
    pub border_crop_back: Option<String>, // Back image for double-faced cards
    pub meld_result: Option<String>,   // For meld cards
}
```

### PDF Generation Constants
- Image dimensions: 480x680 pixels (standard Magic card aspect ratio)
- PDF page size: A4 (210mm x 297mm)
- Layout: 3x3 cards per page (1440x2040 pixels total)
- Physical card size: 6.35cm x 8.7cm at 300 DPI

### PDF Layout Logic
- Uses `printpdf::ImageTransform` for positioning and scaling
- Centers 3x3 grid on A4 page with margins
- Each card scaled to maintain aspect ratio
- Supports multiple pages for large card lists

## Project Structure

This is a Rust workspace with multiple crates:

### Core Library (`magic-proxy-core/`)
- `src/lib.rs` - Main ProxyGenerator API and public interface
- `src/scryfall/` - Scryfall API integration
  - `client.rs` - HTTP client with rate limiting
  - `models.rs` - Card data structures  
  - `api.rs` - API endpoint implementations
- `src/pdf/mod.rs` - PDF generation and layout logic
- `src/cache/mod.rs` - Image caching system
- `src/error.rs` - Error types and conversions

### GUI Application (`magic-proxy-gui/`)
- `src/main.rs` - Application entry point 
- `src/app.rs` - Iced application implementation (incomplete)

### CLI Example (`magic-proxy-cli/`)
- `src/main.rs` - Command-line interface demonstrating core functionality

## Usage Examples

### CLI Tool
```bash
# Search for cards
cargo run --package magic-proxy-cli -- search "Lightning Bolt"

# Generate PDF (when implemented)
cargo run --package magic-proxy-cli -- generate --cards="Lightning Bolt,Counterspell" --output=proxies.pdf
```

### Core Library API
```rust
use magic_proxy_core::{ProxyGenerator, PdfOptions};

let mut generator = ProxyGenerator::new()?;
let results = generator.search_card("Lightning Bolt").await?;
if let Some(card) = results.cards.first() {
    generator.add_card(card.clone(), 4);
}
let pdf_data = generator.generate_pdf(PdfOptions::default(), |current, total| {
    println!("Progress: {}/{}", current, total);
}).await?;
```