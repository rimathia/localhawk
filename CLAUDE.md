# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

LocalHawk Card Sheet Generator - A cross-platform application that creates PDF card sheets for trading cards. Available as both a desktop GUI (Rust/iced) and native iOS app (SwiftUI + Rust core). Users specify a list of cards and the application generates a printable PDF with card images arranged in a grid layout.

### Key Features
- **Cross-Platform Support**: Desktop GUI (Rust/iced) and native iOS app (SwiftUI) sharing the same Rust core
- **Intelligent Double-Faced Card Handling**: Automatically detects when users want specific faces (front vs back) based on input
- **Fuzzy Name Matching**: Advanced card name resolution with support for split cards and alternative names
- **Set/Language Support**: Parse set codes and language specifications in decklists according to what occurs in scryfall
- **Caching**: Caches images, card search results, card names, and set codes
- **Meld Card Support**: Handles Magic meld cards (Gisela/Bruna → Brisela) with proper resolution and display
- **iOS Integration**: Native share sheet support for printing, saving, and sharing generated PDFs

## iOS Native App Implementation

**Status**: ✅ COMPLETED - Native iPad application using SwiftUI + Rust FFI architecture

### Architecture
- **SwiftUI for UI** + **Rust core via FFI** for 100% code reuse of PDF generation logic
- **Memory Management**: Rust allocates with `malloc`, Swift frees with dedicated `localhawk_free_buffer()` 
- **Error Handling**: C-style error codes with descriptive message functions
- **Native iOS Integration**: Share sheet, AirPrint, Files app, background cache persistence

### Key FFI Functions (`localhawk-core/src/ffi.rs`)
- `localhawk_initialize()` - Initialize caches (required first call)
- `localhawk_generate_pdf_from_decklist()` - Main PDF generation
- `localhawk_free_buffer()` - Memory cleanup
- `localhawk_get_*_cache_stats()` - Cache statistics for Advanced Options
- `localhawk_clear_image_cache()` / `localhawk_update_card_names()` - Cache management

### Build System
```bash
./build_ios.sh                                    # Build iOS static libraries
cd LocalHawkiOS && open LocalHawkiOS.xcodeproj  # Open in Xcode
```

**Build Artifacts**:
- `ios-libs/liblocalhawk_core_device.a` - Physical iOS devices
- `ios-libs/liblocalhawk_core_sim.a` - Universal simulator library
- `ios-libs/localhawk.h` - C header for Swift bridging

### iOS App Features
- **Main Interface**: Text editor for decklist input with native share sheet integration
- **Advanced Options**: Cache statistics display and management (gear icon in navigation)
- **Cache Management**: Real-time statistics, clear image cache, update card names database
- **Mobile-Optimized UI**: Expandable path display, color-coded cache cards, smooth animations
- **Background Loading**: Fire-and-forget automatic image loading using `localhawk_parse_and_start_background_loading()`

### Current iOS Limitations (To Be Addressed)
- **Grid Preview Images**: Card images don't yet display in the 3x3 preview grid (placeholders shown)
- **Print Selection**: User cannot yet select different printings (UI framework exists but not functional)
- **Image Display**: GridCardView needs integration with cached image data from background loading

### iOS Threading Architecture
- **Current Approach**: Single-threaded tokio runtime + attempted single-threaded image processing
- **Tokio Runtime**: Uses `new_current_thread()` to inherit caller's QoS and avoid priority inversion
- **Image Processing**: Disabled `jpeg_rayon` feature, but JPEG decoder still has built-in worker threads
- **Known Issue**: JPEG decoder creates worker threads that cause QoS priority inversion warnings
- **Impact**: Functional but generates Thread Performance Checker warnings in Xcode
- **Workarounds**: 
  - Accept the warnings (functionality works correctly)
  - Use alternative image processing library
  - Convert JPEGs to PNG server-side
- **Future Solutions**: Replace jpeg-decoder with single-threaded alternative or implement QoS inheritance

## Key Dependencies

- **iced** - Cross-platform GUI framework (Elm-inspired architecture)
- **reqwest** - HTTP client for Scryfall API calls and image downloads (with `blocking`, `json`, `rustls-tls` features)
- **printpdf** - PDF generation library (with `embedded_images` feature)
- **serde** - JSON serialization for API responses (with `derive` feature)
- **tokio** - Async runtime for concurrent operations (with `time` feature)
- **time** - Date/time handling for cache management (with `serde`, `formatting` features)
- **lazy_static** - For global state management (rate limiting)

## Development Commands

### Build and Run (Desktop)
- `cargo build` - Compile the project
- `cargo run -p localhawk-gui` - Build and run the GUI application
- `cargo run -p localhawk-cli` - Build and run the CLI application
- `cargo check` - Check for compilation errors without building

### Build and Run (iOS)
- `./build_ios.sh` - Build iOS static libraries for device and simulator
- `cd LocalHawkiOS && xcodebuild -project LocalHawkiOS.xcodeproj -scheme LocalHawkiOS -destination 'platform=iOS Simulator,name=iPad Air 11-inch (M3)' build` - Build iOS app
- `xcrun simctl install "iPad Air 11-inch (M3)" "./path/to/LocalHawkiOS.app"` - Install on simulator
- `xcrun simctl launch "iPad Air 11-inch (M3)" com.localhawk.LocalHawkiOS` - Launch iOS app
- `open -a Simulator` - Open iOS Simulator for manual testing
- `cd LocalHawkiOS && open LocalHawkiOS.xcodeproj` - Open in Xcode for development

### Testing and Quality
- `cargo test` - Run all tests
- `cargo clippy` - Run Rust linter
- `cargo fmt` - Format code according to Rust standards

### Development Workflow
- `cargo clean` - Remove build artifacts from target directory
- `cargo update` - Update dependencies in Cargo.lock
- **ALWAYS run `cargo fmt` as the final step after making any code changes** - This ensures consistent code formatting across the entire codebase

### Compilation Verification (CRITICAL)
**ALWAYS verify code compiles after making changes** - This prevents presenting broken code to users and maintains development flow

#### Desktop (Rust)
- `cargo check` - Fast compilation check without building
- `cargo build` - Full compilation verification
- `cargo test` - Ensure tests still pass after changes

#### iOS App (Swift + Rust FFI)
- `./build_ios.sh` - Rebuild Rust libraries for iOS
- `cd LocalHawkiOS && xcodebuild -project LocalHawkiOS.xcodeproj -scheme LocalHawkiOS -destination 'platform=iOS Simulator,name=iPad Air 11-inch (M3)' build` - Verify Swift code compiles
- **Critical for FFI changes**: Both Rust and Swift sides must be verified

#### Workflow
1. Make code changes
2. **Immediately verify compilation** using appropriate commands above
3. Fix any compilation errors before proceeding
4. Format code with `cargo fmt`
5. Only then present working code

**Never present code changes without compilation verification** - Compilation failures break user workflow and waste development time

#### 🛑 PROCESS ENFORCEMENT
**The following are BLOCKING requirements - if you skip them, STOP immediately:**

1. **Before Layout/UI Changes**: Must commit working state first (see Git Workflow section)
2. **After Code Changes**: Must verify compilation before presenting to user  
3. **Complex Changes**: Must use TodoWrite to track steps and ensure nothing is skipped

**Why written instructions fail**: Good intentions + documentation ≠ reliable behavior without forcing functions. These blocking requirements create necessary workflow interruptions.

## Code Style Guidelines

### Naming Conventions
- **Underscore prefix (_) for unused entities only** - Names starting with underscore should only be used for variables, functions, fields, or other entities that are intentionally unused
- **Remove unused code** - If code is no longer needed, remove it entirely rather than prefixing with underscore
- **Temporary underscore prefix** - Only use underscore prefix as a temporary measure during development when you know code will be used later
- Examples:
  - ✅ `_response` for an unused function parameter
  - ✅ `_future_field` for a struct field reserved for future use
  - ❌ `_calculate_total_pages` for a method that was used but is now dead code (remove instead)
  - ❌ `_helper_function` for a function that is actively called (rename to `helper_function`)

### Git Workflow (CRITICAL - NO EXCEPTIONS)

#### 🛑 MANDATORY PRE-REFACTORING CHECKLIST
**Before ANY of these changes, you MUST commit working state first:**
- Layout changes or UI restructuring 
- Moving code between files/modules
- Changing function signatures or data structures
- Adding/removing large code sections
- Any change that might break existing functionality

**MANDATORY STEPS:**
1. `git status` - Check current state
2. `git add -A && git commit -m "working state: [describe current functionality]"`  
3. ONLY THEN proceed with risky changes

**🚨 FAILURE RECOVERY**: If you ignored this and broke working code:
- Stop immediately, don't try to "fix forward"  
- Check `git status` for any recovery options
- Consider reverting and starting over with proper process

#### Commit Guidelines  
- Use descriptive commit messages that explain what is working at that state
- Example messages: "print selection working with image display", "background loading functional", "layout responsive and buttons visible"
- Every commit should represent a stable, working state
- Commit frequency: Better to over-commit than under-commit during complex changes

#### File Management
- **NEVER add files to version control without asking first** - Always verify whether a file should be tracked before committing
- **Ask before adding any new files**: Debug logs, temporary files, build artifacts, cache files, or any file you're unsure about
- Examples of files to avoid: `debug.log`, `*.tmp`, `.DS_Store`, `target/*`, cache directories, temporary test files
- When unsure, describe the file and ask: "Should I include this file in the commit?"

#### GUI Application Workflow
- **This is a graphical application** - Visual and interactive functionality cannot be verified through code review alone
- **Always seek user feedback before committing GUI changes** - The user needs to test the interface to confirm it works as expected
- **Wait for user confirmation** before marking GUI features as "working" or "complete"
- **Ask before committing**: "Should I commit this state?" or "Does this meet your expectations?"
- Examples requiring user verification: Layout changes, visual alignment, interactive behavior, styling, user workflows
- Code compilation success ≠ feature completeness for GUI applications

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
  - **Known limitation**: Some special event cards (e.g., "Bad Knight" from Unknown Event set) appear in the card names catalog but return 404 when searched via the API

## Implementation Patterns from MagicHawk

### Rate Limiting
- Use `lazy_static` Mutex to track last API call timestamp
- Enforce 100ms cooldown between Scryfall API requests
- Both async and blocking variants available

### Background Image Loading
- **Progressive Loading**: Images load sequentially, starting with current PDF images then alternative printings
- **Rate-Limited**: Respects existing 10 requests/second Scryfall limit
- **Cache Integration**: Uses existing image cache system, stores raw bytes for GUI efficiency
- **Non-Blocking**: Background tasks don't interfere with UI responsiveness
- **Error Handling**: Failed loads don't block successful images

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
    pub face_mode: DoubleFaceMode,     // Fully resolved face preference (replaced preferred_face)
    pub source_line_number: Option<usize>, // Line number in original decklist for debugging
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

## Architecture: Single-Source-of-Truth for Face Mode Resolution

### Problem Solved
The application previously had a potential inconsistency where the grid preview and PDF generation could show different results for double-faced cards. This occurred because face preferences were resolved at different times and potentially with different logic.

### Solution: Parse-Time Resolution
The architecture now uses a **single-source-of-truth** approach:

1. **Parse-Time Resolution**: Face preferences are fully resolved during `parse_and_resolve_decklist()` and stored in `DecklistEntry.face_mode`
2. **Global Face Mode Input**: The parsing function accepts the current global face mode setting as a parameter
3. **Consistent Application**: Both grid preview and PDF generation use the same resolved face modes

### API Changes
```rust
// NEW: Takes global face mode as parameter and resolves face preferences immediately
pub async fn parse_and_resolve_decklist(
    decklist_text: &str,
    global_face_mode: DoubleFaceMode,
) -> Result<Vec<DecklistEntry>, ProxyError>

// Each DecklistEntry now contains fully resolved face mode
pub struct DecklistEntry {
    // ... other fields ...
    pub face_mode: DoubleFaceMode,  // Resolved during parsing
}
```

### Face Mode Resolution Logic
Applied during parsing in `parse_and_resolve_decklist()`:
- **Back face input** (`Part(1)` from fuzzy matching) → Always `BackOnly` (overrides global)
- **Front face input** or **full card name** → Uses the provided global face mode
- **No match found** → Uses the provided global face mode

### Shared Logic for Consistency
```rust
// Shared helper function used by both grid preview and PDF generation
pub fn get_image_urls_for_face_mode(card: &Card, face_mode: &DoubleFaceMode) -> Vec<String>
```

This ensures that:
- Grid preview shows exactly what the PDF will contain
- No timing issues with changing global settings after parsing
- Single implementation of face mode logic eliminates duplication
- PDF generation logic remains unchanged (minimal regression risk)

## Cache System

The application uses a sophisticated multi-layered caching system optimized for performance and reliability:

### Cache Types and Strategies

#### 1. Image Cache (`ImageCache`)
- **Purpose**: Stores downloaded card images to avoid repeated network requests
- **Location**: `~/.cache/localhawk/` (platform-specific cache directory)
- **Format**: Raw JPEG/PNG bytes with SHA256-hashed filenames + JSON metadata
- **Storage Strategy**: Stores raw bytes as downloaded from Scryfall, converts to `DynamicImage` on-demand for PDF generation
- **GUI Access**: `get_cached_image_bytes()` provides raw bytes for direct use with `iced::widget::Image`
- **Size Limit**: 1 GB by default (`DEFAULT_MAX_SIZE_MB = 1000`)
- **Eviction**: LRU (Least Recently Used) when cache exceeds size limit
- **Persistence Strategy**:
  - **Runtime**: Pure in-memory operations (no disk I/O)
  - **Startup**: Load metadata and existing images from disk
  - **Shutdown**: Save metadata to disk via `shutdown_caches()`
  - **Clear**: Immediate disk cleanup when explicitly cleared

#### 2. Search Results Cache (`SearchResultsCache`)
- **Purpose**: Cache Scryfall API search responses to reduce API calls
- **Location**: `~/.cache/localhawk/search_results.json`
- **Validity**: Permanent (search results don't change for card names)
- **Access Tracking**: Updates `last_accessed` timestamp for each cached search
- **Persistence Strategy**:
  - **Runtime**: Pure in-memory operations
  - **Startup**: Load all cached searches from disk
  - **Shutdown**: Save all new searches to disk via `shutdown_caches()`

#### 3. Card Names Cache (`CardNameCache`)
- **Purpose**: Stores complete Scryfall card names catalog for fuzzy matching
- **Location**: `~/.cache/localhawk/card_names.json`
- **Validity**: 1 day (`CACHE_DURATION_DAYS = 1`)
- **Data**: ~32,000+ card names with timestamp
- **Persistence Strategy**:
  - **Startup**: Check if cache is < 1 day old, fetch from API if expired
  - **Runtime**: Pure in-memory fuzzy matching
  - **Force Update**: Immediate save to disk when user requests refresh
  - **Automatic Expiration**: Next startup will fetch fresh data if > 1 day old

#### 4. Set Codes Cache (`SetCodesCache`)
- **Purpose**: Stores all Magic set codes for decklist parsing
- **Location**: `~/.cache/localhawk/set_codes.json`
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

### Core Library (`localhawk-core/`)
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

### GUI Application (`localhawk-gui/`)
- `src/main.rs` - Application entry point with cache initialization
- `src/app.rs` - Complete Iced application with:
  - **Grid Preview System**: 3x3 preview grids showing actual card images when cached
  - **Print Selection Modal**: Browse alternative printings with thumbnail images
  - **Background Image Loading**: Progressive image loading with rate limiting
  - **Entry-Based Print Selection**: One print choice per decklist entry affects all copies
  - **Page Navigation**: Multi-page navigation for large decklists
  - **Double-Faced Card Support**: Intelligent face detection and mode selection
  - **Expandable Advanced Options Sidebar**: Toggleable sidebar with card name database and image cache management

### CLI Example (`localhawk-cli/`)
- `src/main.rs` - Command-line interface demonstrating core functionality

## Usage Examples

### CLI Tool
```bash
# Search for cards
cargo run -p localhawk-cli -- search "Lightning Bolt"

# Generate PDF (when implemented)
cargo run -p localhawk-cli -- generate --cards="Lightning Bolt,Counterspell" --output=proxies.pdf
```

### Core Library API
```rust
use localhawk_core::{ProxyGenerator, PdfOptions, DoubleFaceMode, initialize_caches, shutdown_caches};

// Initialize caches at startup (required)
initialize_caches().await?;

// Parse decklist with intelligent face detection (requires global face mode)
let decklist = "1 kabira takedown\n1 kabira plateau\n1 cut // ribbons";
let global_face_mode = DoubleFaceMode::BothSides;
let entries = ProxyGenerator::parse_and_resolve_decklist(decklist, global_face_mode).await?;

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
- **Unit tests**: 45+ tests passing, comprehensive coverage of all functionality
- **Face Mode Resolution Testing**: Complete test coverage ensuring architecture consistency
  - Tests all three face modes (`FrontOnly`, `BackOnly`, `BothSides`) with MagicHawk logic
  - Verifies back face input (`Part(1)`) always forces `BackOnly` regardless of global setting
  - Confirms front face and full name inputs use global face mode setting
  - Uses controlled card name data to avoid external dependencies
- **Self-contained requirement**: All unit tests MUST be self-contained and executable in restrictive CI environments
  - No network calls (use mocked data instead of real Scryfall API calls)
  - No external dependencies or services
  - Deterministic results (not dependent on changing external data)
  - Fast execution (tests complete in milliseconds, not seconds)
- **Cache persistence test**: Disabled (`#[ignore]`) due to file system dependencies
  - TODO: Refactor to use dependency injection with in-memory storage for unit tests
  - Consider moving file system tests to integration tests
- **Integration tests**: None implemented yet, but may be added later for end-to-end testing with real external services

### Decklist Parsing
- **Set codes**: Supports 2-6 character codes (regex: `[\dA-Za-z]{2,6}`)
- **Examples**: "BRO", "PLST", "PMPS08", "30A", "H2R"
- **Language codes**: Standard 2-letter codes (JA, FR, DE, etc.)
- **Format**: `4 Lightning Bolt [BRO]` or `1 Memory Lapse [JA]`

### Scryfall Search Improvements
- **Exact name matching**: Filters API results to match requested card name exactly
- **Proper URL encoding**: Handles special characters like "//" in card names
- **Result filtering**: Only returns cards that match the search criteria

## Desktop GUI Features

### Multi-Page Grid Preview with Print Selection
**Status**: ✅ IMPLEMENTED - Visual PDF preview with per-card print selection capabilities

#### Core Features
- **Entry-Based Selection**: One print choice per decklist entry affects all copies (`4x Lightning Bolt` = one selection)
- **3x3 Grid Preview**: Exact PDF page layout preview with page navigation
- **Print Selection Modal**: 4x4 thumbnail grid showing all available printings with set/language overlays
- **Set Hint Integration**: `[LEA]` in decklist becomes default selection in print picker

#### Key Data Structures
```rust
pub struct GridPreview {
    pub entries: Vec<PreviewEntry>,           // One per decklist entry
    pub current_page: usize,                  // Page navigation
    pub selected_entry_index: Option<usize>, // For print selection modal
}

pub struct PreviewEntry {
    pub decklist_entry: DecklistEntry,       // Original entry
    pub available_printings: Vec<Card>,      // All printings from search
    pub selected_printing: Option<usize>,    // User's choice
    pub grid_positions: Vec<GridPosition>,   // Card positions on pages
}
```

#### Workflow Integration
**Enhanced**: `Parse Decklist → Preview Pages → [Optional: Customize Prints] → Generate PDF → Save`

### Advanced Options Sidebar
- **Toggleable sidebar** (480px) with card name database and image cache management
- **Cache Statistics**: Real-time display of counts, sizes, and cache paths
- **Visual Design**: Color-coded sections with consistent typography and smooth animations

## Architecture Notes

### Single-Source-of-Truth for Face Mode Resolution
**Problem Solved**: Potential inconsistency between grid preview and PDF generation for double-faced cards

**Solution**: Face preferences are fully resolved during `parse_and_resolve_decklist()` and stored in `DecklistEntry.face_mode`. Both grid preview and PDF generation use the same resolved face modes, ensuring identical results.

**Key Changes**:
- `DecklistEntry` now contains `face_mode: DoubleFaceMode` (fully resolved)
- `parse_and_resolve_decklist()` accepts global face mode parameter
- Shared `get_image_urls_for_face_mode()` helper ensures consistency

### Known Limitations
- **Background Loader Synchronization**: Background image loader doesn't update when users select different printings in grid preview
- **Cache Persistence Test**: Disabled due to file system dependencies in unit tests
