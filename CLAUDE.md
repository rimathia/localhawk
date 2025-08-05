# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Magic Card Proxy Sheet Generator - A Rust GUI application that creates PDF proxy sheets for Magic: The Gathering cards. Users specify a list of cards and the application generates a printable PDF with card images arranged in a grid layout.

### Key Features
- **Intelligent Double-Faced Card Handling**: Automatically detects when users want specific faces (front vs back) based on input
- **Fuzzy Name Matching**: Advanced card name resolution with support for split cards and alternative names  
- **Flexible Set/Language Support**: Parse set codes (2-6 characters) and language specifications in decklists
- **Comprehensive Caching**: Multi-layer caching for images, search results, card names, and set codes

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
pub struct Card {
    pub name: String,
    pub set: String, 
    pub language: String,
    pub border_crop: String,           // Front image URL
    pub border_crop_back: Option<String>, // Back image for double-faced cards
    pub meld_result: Option<String>,   // For meld cards
}
```

### Decklist Entry Model
```rust
pub struct DecklistEntry {
    pub multiple: i32,
    pub name: String,
    pub set: Option<String>,           // Parsed from [SET] notation
    pub lang: Option<String>,          // Parsed from [LANG] notation  
    pub preferred_face: Option<NameMatchMode>, // Tracks which face was matched
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

### Double-Faced Card Options
```rust
pub enum DoubleFaceMode {
    FrontOnly,    // Include only front face of double-faced cards
    BackOnly,     // Include only back face of double-faced cards  
    BothSides,    // Include both faces as separate cards (default)
}
```

#### Intelligent Face Detection
- **Front face input** (e.g., "kabira takedown") → Uses global `DoubleFaceMode` setting
- **Back face input** (e.g., "kabira plateau") → Always uses `BackOnly` mode (overrides global)
- **Full card name** (e.g., "cut // ribbons") → Uses global `DoubleFaceMode` setting

This allows users to mix entries in the same decklist where some should show both faces and others should show only specific faces.

## Cache System

The application uses a sophisticated multi-layered caching system optimized for performance and reliability:

### Cache Types and Strategies

#### 1. Image Cache (`ImageCache`)
- **Purpose**: Stores downloaded card images to avoid repeated network requests
- **Location**: `~/.cache/magic-proxy/` (platform-specific cache directory)
- **Format**: JPEG files with SHA256-hashed filenames + JSON metadata
- **Size Limit**: 1 GB by default (`DEFAULT_MAX_SIZE_MB = 1000`)
- **Eviction**: LRU (Least Recently Used) when cache exceeds size limit
- **Persistence Strategy**: 
  - **Runtime**: Pure in-memory operations (no disk I/O)
  - **Startup**: Load metadata and existing images from disk
  - **Shutdown**: Save metadata to disk via `shutdown_caches()`
  - **Clear**: Immediate disk cleanup when explicitly cleared

#### 2. Search Results Cache (`SearchResultsCache`)
- **Purpose**: Cache Scryfall API search responses to reduce API calls
- **Location**: `~/.cache/magic-proxy/search_results.json`
- **Validity**: Permanent (search results don't change for card names)
- **Access Tracking**: Updates `last_accessed` timestamp for each cached search
- **Persistence Strategy**:
  - **Runtime**: Pure in-memory operations
  - **Startup**: Load all cached searches from disk
  - **Shutdown**: Save all new searches to disk via `shutdown_caches()`

#### 3. Card Names Cache (`CardNameCache`)
- **Purpose**: Stores complete Scryfall card names catalog for fuzzy matching
- **Location**: `~/.cache/magic-proxy/card_names.json`
- **Validity**: 1 day (`CACHE_DURATION_DAYS = 1`)
- **Data**: ~32,000+ card names with timestamp
- **Persistence Strategy**:
  - **Startup**: Check if cache is < 1 day old, fetch from API if expired
  - **Runtime**: Pure in-memory fuzzy matching
  - **Force Update**: Immediate save to disk when user requests refresh
  - **Automatic Expiration**: Next startup will fetch fresh data if > 1 day old

#### 4. Set Codes Cache (`SetCodesCache`)
- **Purpose**: Stores all Magic set codes for decklist parsing
- **Location**: `~/.cache/magic-proxy/set_codes.json`
- **Validity**: 1 day (`CACHE_DURATION_DAYS = 1`) - matches card names for consistency
- **Data**: ~1,000 set codes (e.g., "lea", "leb", "2ed", etc.)
- **Persistence Strategy**: Same as Card Names Cache

### Global Singleton Pattern

All caches use thread-safe global singletons via `OnceLock<Arc<RwLock<T>>>`:

```rust
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<ImageCache>>> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<Arc<RwLock<SearchResultsCache>>> = OnceLock::new();
static SET_CODES_CACHE: OnceLock<Arc<RwLock<Option<HashSet<String>>>>> = OnceLock::new();
```

### Cache Initialization and Shutdown

#### Startup (`initialize_caches()`)
- Must be called at application startup (both GUI and CLI)
- Loads all disk caches into memory
- Checks validity for card names and set codes (1-day expiration)
- Builds fuzzy matching index (~450ms for 32K+ card names)
- Creates tokio runtime for async operations in GUI

#### Shutdown (`shutdown_caches()`)
- Call before application exit to persist changes
- Saves image cache metadata and search results to disk
- Card names and set codes already saved when updated
- Ensures no data loss on clean exit

### Performance Characteristics

- **Startup**: One-time disk reads and network requests (if expired)
- **Runtime**: Pure in-memory operations, no disk I/O or blocking
- **Memory Usage**: ~50-100MB for all caches combined
- **Network**: Only on cache misses or expiration
- **Disk**: Only on startup, shutdown, and explicit operations

### Error Handling

- **Disk Errors**: Graceful degradation, cache operates in memory-only mode
- **Network Errors**: Falls back to existing cache when possible
- **Corruption**: Invalid cache files are ignored, fresh data fetched
- **Thread Safety**: All operations are thread-safe via RwLock

## Project Structure

This is a Rust workspace with multiple crates:

### Core Library (`magic-proxy-core/`)
- `src/lib.rs` - Main ProxyGenerator API and public interface
- `src/scryfall/` - Scryfall API integration
  - `client.rs` - HTTP client with rate limiting
  - `models.rs` - Card data structures  
  - `api.rs` - API endpoint implementations (with exact name matching)
- `src/pdf/mod.rs` - PDF generation and layout logic with DoubleFaceMode support
- `src/decklist/mod.rs` - Decklist parsing with set/language detection (2-6 char set codes)
- `src/lookup.rs` - Fuzzy name matching with split/double-faced card support
- `src/cache/mod.rs` - Image caching system
- `src/search_results_cache.rs` - Scryfall search result caching
- `src/card_name_cache.rs` - Card names catalog caching
- `src/set_codes_cache.rs` - Magic set codes caching
- `src/globals.rs` - Global cache management and initialization
- `src/error.rs` - Error types and conversions

### GUI Application (`magic-proxy-gui/`)
- `src/main.rs` - Application entry point with cache initialization
- `src/app.rs` - Complete Iced application with double-faced card dropdown and intelligent detection

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
use magic_proxy_core::{ProxyGenerator, PdfOptions, DoubleFaceMode, initialize_caches, shutdown_caches};

// Initialize caches at startup (required)
initialize_caches().await?;

// Parse decklist with intelligent face detection
let decklist = "1 kabira takedown\n1 kabira plateau\n1 cut // ribbons";
let entries = ProxyGenerator::parse_and_resolve_decklist(decklist).await?;

// Generate PDF with per-card face modes
let cards: Vec<(Card, u32, DoubleFaceMode)> = /* ... build from entries ... */;
let pdf_options = PdfOptions { 
    double_face_mode: DoubleFaceMode::BothSides,
    ..Default::default() 
};
let pdf_data = ProxyGenerator::generate_pdf_from_cards_with_face_modes(
    &cards, pdf_options, |current, total| {
        println!("Progress: {}/{}", current, total);
    }
).await?;

// Clean shutdown to persist caches
shutdown_caches().await?;
```

## Important Notes

### Testing
- **Unit tests**: 42 tests passing, comprehensive coverage of all functionality
- **Cache persistence test**: Disabled (`#[ignore]`) due to file system dependencies
  - TODO: Refactor to use dependency injection with in-memory storage for unit tests
  - Consider moving file system tests to integration tests

### Decklist Parsing
- **Set codes**: Supports 2-6 character codes (regex: `[\dA-Za-z]{2,6}`)
- **Examples**: "BRO", "PLST", "PMPS08", "30A", "H2R" 
- **Language codes**: Standard 2-letter codes (JA, FR, DE, etc.)
- **Format**: `4 Lightning Bolt [BRO]` or `1 Memory Lapse [JA]`

### Scryfall Search Improvements
- **Exact name matching**: Filters API results to match requested card name exactly
- **Proper URL encoding**: Handles special characters like "//" in card names
- **Result filtering**: Only returns cards that match the search criteria