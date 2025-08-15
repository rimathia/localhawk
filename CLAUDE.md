# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Magic Card Proxy Sheet Generator - A cross-platform application that creates PDF proxy sheets for Magic: The Gathering cards. Available as both a desktop GUI (Rust/iced) and native iOS app (SwiftUI + Rust core). Users specify a list of cards and the application generates a printable PDF with card images arranged in a grid layout.

### Key Features
- **Cross-Platform Support**: Desktop GUI (Rust/iced) and native iOS app (SwiftUI) sharing the same Rust core
- **Intelligent Double-Faced Card Handling**: Automatically detects when users want specific faces (front vs back) based on input
- **Fuzzy Name Matching**: Advanced card name resolution with support for split cards and alternative names  
- **Flexible Set/Language Support**: Parse set codes (2-6 characters) and language specifications in decklists
- **Comprehensive Caching**: Multi-layer caching for images, search results, card names, and set codes
- **Meld Card Support**: Handles Magic meld cards (Gisela/Bruna → Brisela) with proper resolution and display
- **iOS Integration**: Native share sheet support for printing, saving, and sharing generated PDFs

## Current Issue: Meld Card Bug Investigation (January 9, 2025)

**Status**: ✅ RESOLVED - Fixed pagination logic for meld cards

### Problem (FIXED)
- ~~Gisela shows correctly: Gisela front + Brisela meld result ✅~~
- ~~Bruna shows incorrectly: Bruna front + Bruna front (instead of Brisela) ❌~~
- **Fix Applied**: Corrected grid preview pagination logic to properly handle meld result images across page boundaries

### Root Cause Analysis (Completed)
1. **✅ Meld resolution logic works**: API calls succeed, debug logs confirm "Found meld result 'brisela, voice of nightmares'"
2. **✅ Scryfall API returns correct data**: Direct API query shows proper Brisela cards with correct image URLs
3. **❌ Bug in result selection logic**: The issue is in `resolve_meld_result()` set matching/fallback logic

### Technical Details
**Problem Location**: `magic-proxy-core/src/scryfall/api.rs:137-142`
```rust
let meld_card = meld_search_result.cards
    .iter()
    .find(|meld_card| meld_card.set == card.set)
    .or_else(|| meld_search_result.cards.first()) // BUG: May select wrong card
```

**Issue**: When searching for "brisela, voice of nightmares", somehow Bruna cards are getting into `meld_search_result.cards` despite the exact name filtering in `search_card_internal()`. The `.first()` fallback then selects Bruna instead of Brisela.

### Evidence
- **Cache data shows**: Bruna's `meld_result_image_url` = `6fccdb60-5fce-4a6e-a709-b986f9a4b653.jpg` (Bruna's front image)
- **API returns**: Only legitimate Brisela cards with proper Brisela image URLs
- **Conclusion**: Name filtering logic is failing, allowing wrong cards through

### Next Steps
1. **Debug the exact name filtering** in `search_card_internal()` - why are Bruna cards passing the filter?
2. **Fix result selection logic** - ensure fallback only selects cards with correct names
3. **Add validation** - verify selected meld result has expected name before assignment
4. **Test thoroughly** with both Gisela and Bruna to confirm fix

### Files Involved
- `magic-proxy-core/src/scryfall/api.rs` - Meld resolution logic (lines 120-155)
- `magic-proxy-core/src/scryfall/models.rs` - Card data model with BackSide enum
- Search results cache: `/Users/mathiasritzmann/Library/Caches/magic-proxy/search_results_cache.json`

## iOS Native App Implementation (August 15, 2025)

**Status**: ✅ COMPLETED - Native iOS app successfully created and tested

### Overview
Successfully implemented a native iPad application using SwiftUI + Rust FFI architecture. The iOS app shares the same core PDF generation logic as the desktop application while providing a native iOS user experience.

### Architecture Decision: SwiftUI + Rust Core

**Approach Chosen**: SwiftUI for UI + Rust core via FFI (Foreign Function Interface)

**Alternative Approaches Considered**:
- Progressive Web App (PWA) - Abandoned due to printpdf library lacking WASM support
- Tauri Mobile - Less mature ecosystem
- React Native - Would require JavaScript bridge
- Buck2 build system - Too early for external users (no stable releases)

**Why SwiftUI + Rust Core**:
- **Code Reuse**: 100% of PDF generation logic shared between desktop and mobile
- **Native Performance**: Full native iOS performance and integration
- **App Store Compatible**: Can be distributed via App Store
- **Platform Features**: Access to native iOS sharing, printing, and file management
- **Maintainability**: Single source of truth for core business logic

### Technical Implementation

#### FFI Layer (`magic-proxy-core/src/ffi.rs`)
```rust
#[unsafe(no_mangle)]
pub extern "C" fn proxy_generate_pdf_from_decklist(
    decklist_cstr: *const c_char,
    output_buffer: *mut *mut u8,
    output_size: *mut usize,
) -> c_int
```

**Memory Management Pattern**:
- **Rust allocates** PDF data using `malloc`
- **Swift frees** using dedicated `proxy_free_buffer()` function
- **Error handling** via C-style error codes with descriptive messages

**Core FFI Functions**:
- `proxy_initialize()` - Initialize caches (required first call)
- `proxy_generate_pdf_from_decklist()` - Main PDF generation
- `proxy_free_buffer()` - Memory cleanup
- `proxy_test_connection()` - Simple connectivity test (returns 42)
- `proxy_get_error_message()` - Human-readable error descriptions

#### Swift Integration (`MagicProxyiOS/MagicProxyiOS/ProxyGenerator.swift`)
```swift
static func generatePDF(from decklist: String) -> Result<Data, ProxyGeneratorError> {
    // Initialize, call FFI, handle memory management
    let data = Data(bytes: buffer, count: size)
    proxy_free_buffer(buffer) // Critical: prevent memory leaks
    return .success(data)
}
```

**Features**:
- Type-safe Swift wrapper around C functions
- Automatic memory management with guard clauses
- Comprehensive error handling with localized messages
- Background thread execution to prevent UI blocking

#### SwiftUI Interface (`MagicProxyiOS/MagicProxyiOS/ContentView.swift`)
```swift
struct ContentView: View {
    @State private var decklistText = "1 Lightning Bolt\n1 Counterspell..."
    @State private var isGenerating = false
    @State private var pdfData: Data?
}
```

**UI Components**:
- **Text Editor**: Monospaced font for decklist input with sample content
- **Generate Button**: Progress indication and disabled state during generation
- **Share Integration**: Native iOS share sheet (`UIActivityViewController`)
- **Error Display**: User-friendly error messages with retry capability
- **FFI Test Display**: Shows "42" when Rust connection is working

### Build System

#### iOS Static Library Generation (`build_ios.sh`)
```bash
# Build for all iOS targets
cargo build --release --target aarch64-apple-ios -p magic-proxy-core          # Device
cargo build --release --target x86_64-apple-ios -p magic-proxy-core           # Simulator x86_64  
cargo build --release --target aarch64-apple-ios-sim -p magic-proxy-core      # Simulator ARM64

# Create universal simulator library
lipo -create \
  target/x86_64-apple-ios/release/libmagic_proxy_core.a \
  target/aarch64-apple-ios-sim/release/libmagic_proxy_core.a \
  -output ios-libs/libmagic_proxy_core_sim.a
```

**Output Files**:
- `ios-libs/libmagic_proxy_core_device.a` - Physical iOS devices
- `ios-libs/libmagic_proxy_core_sim.a` - Universal simulator library
- `ios-libs/magic_proxy.h` - C header for Swift bridging

**Key Build Configuration**:
```toml
[lib]
crate-type = ["cdylib", "staticlib", "rlib"]  # Enable static library generation
```

#### Xcode Project Structure
```
MagicProxyiOS/
├── MagicProxyiOS.xcodeproj/           # Xcode project
│   └── project.pbxproj                # Project configuration
└── MagicProxyiOS/                     # Source code
    ├── MagicProxyiOSApp.swift         # App entry point
    ├── ContentView.swift              # Main UI
    ├── ProxyGenerator.swift           # FFI wrapper
    ├── MagicProxyiOS-Bridging-Header.h # C bridging
    └── Assets.xcassets/               # App icons and colors
```

### iOS Integration Features

#### Native Share Sheet Integration
```swift
sheet(isPresented: $showingShareSheet) {
    if let pdfData = pdfData {
        ShareSheet(items: [pdfData])  // UIActivityViewController wrapper
    }
}
```

**Supported Actions**:
- **Save to Files** - Store in iOS Files app for user access
- **Print** - Direct printing to AirPrint-enabled printers  
- **Mail/Messages** - Share via email or text
- **AirDrop** - Share to nearby Apple devices
- **Save to Books** - View PDFs in Apple Books app

#### File Access Patterns
**App Sandbox**: `/Users/.../Library/Developer/CoreSimulator/Devices/.../data/Containers/Data/Application/.../`
- `Documents/` - App documents (persistent)
- `Library/` - App library files  
- `tmp/` - Temporary files (system cleaned)

**User Access Methods**:
1. **Files App**: "On My iPad" → "MagicProxyiOS" folder
2. **Share Sheet**: User-controlled save locations
3. **Simulator Menu**: Device → Photos/Documents (development only)

### Development Workflow

#### Building and Testing
```bash
# 1. Build iOS static libraries
./build_ios.sh

# 2. Build iOS app for simulator
cd MagicProxyiOS
xcodebuild -project MagicProxyiOS.xcodeproj -scheme MagicProxyiOS \
  -destination 'platform=iOS Simulator,name=iPad Air 11-inch (M3)' build

# 3. Install and run on simulator
xcrun simctl install "iPad Air 11-inch (M3)" "./path/to/MagicProxyiOS.app"
xcrun simctl launch "iPad Air 11-inch (M3)" com.magicproxy.MagicProxyiOS
```

#### Opening for Development
```bash
# Option 1: Open Xcode project
cd MagicProxyiOS && open MagicProxyiOS.xcodeproj

# Option 2: Open simulator directly
open -a Simulator
```

### Key Design Decisions

#### Memory Management
**Decision**: Rust allocates with `malloc`, Swift frees with dedicated function
**Rationale**: 
- Standard C pattern familiar to iOS developers
- Clear ownership boundaries (Rust creates, Swift destroys)
- Prevents memory leaks through dedicated cleanup function
- Compatible with Swift's automatic reference counting

#### Error Handling Strategy
**Decision**: C-style error codes with separate message function
**Rationale**:
- Simple FFI interface (integers cross language boundaries easily)
- Detailed error messages available when needed
- Swift can wrap in proper `Result<T, Error>` types
- Compatible with iOS error handling patterns

#### Face Mode Default
**Decision**: `DoubleFaceMode::BothSides` for iOS
**Rationale**:
- Mobile users likely want to see both sides of double-faced cards
- Consistent with desktop default behavior
- Can be customized in future versions if needed

#### UI Design Philosophy
**Decision**: Native SwiftUI with iOS design patterns
**Rationale**:
- Feels natural to iOS users (not like a ported desktop app)
- Leverages iOS share sheet for maximum compatibility
- Uses standard iOS navigation and layout patterns
- Optimized for iPad screen sizes and touch interaction

### Testing and Verification

#### Functional Testing
- ✅ **FFI Connection**: Test function returns 42 (connectivity verified)
- ✅ **PDF Generation**: Sample decklists generate valid PDFs
- ✅ **Error Handling**: Invalid input produces user-friendly error messages
- ✅ **Share Sheet**: PDFs successfully shared via all iOS mechanisms
- ✅ **Memory Management**: No memory leaks during PDF generation cycles

#### Performance Testing
- ✅ **Generation Speed**: Comparable to desktop version (same core logic)
- ✅ **UI Responsiveness**: Background generation prevents UI blocking
- ✅ **Memory Usage**: Efficient memory cleanup after PDF generation

### Current Limitations and Future Enhancements

#### Current Limitations
- **Cache Location**: Uses default cache directory (not user-configurable)
- **Face Mode**: Fixed to `BothSides` (no user setting)
- **Image Preview**: No grid preview (desktop-only feature)
- **Set Selection**: No alternative printing selection

#### Future Enhancements
- **Settings Screen**: Allow cache management and face mode selection
- **Grid Preview**: Port desktop preview functionality to SwiftUI
- **Print Selection**: Alternative card printings selection
- **Background Sync**: Cache warm-up on app launch
- **iPhone Support**: Adapt UI for smaller screens

### Files and Structure

#### Core FFI Files
- `magic-proxy-core/src/ffi.rs` - C-compatible functions for iOS
- `magic-proxy-core/include/magic_proxy.h` - C header for Swift bridging
- `build_ios.sh` - iOS static library build script

#### iOS Application Files  
- `MagicProxyiOS/MagicProxyiOS/MagicProxyiOSApp.swift` - App entry point and initialization
- `MagicProxyiOS/MagicProxyiOS/ContentView.swift` - Main SwiftUI interface
- `MagicProxyiOS/MagicProxyiOS/ProxyGenerator.swift` - Swift FFI wrapper
- `MagicProxyiOS/MagicProxyiOS/MagicProxyiOS-Bridging-Header.h` - C to Swift bridge
- `MagicProxyiOS/MagicProxyiOS.xcodeproj/project.pbxproj` - Xcode project configuration

#### Generated Build Artifacts
- `ios-libs/libmagic_proxy_core_device.a` - Device static library
- `ios-libs/libmagic_proxy_core_sim.a` - Simulator static library  
- `ios-libs/magic_proxy.h` - C header file for integration

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
- `cargo run -p magic-proxy-gui` - Build and run the GUI application
- `cargo run -p magic-proxy-cli` - Build and run the CLI application
- `cargo check` - Check for compilation errors without building

### Build and Run (iOS)
- `./build_ios.sh` - Build iOS static libraries for device and simulator
- `cd MagicProxyiOS && xcodebuild -project MagicProxyiOS.xcodeproj -scheme MagicProxyiOS -destination 'platform=iOS Simulator,name=iPad Air 11-inch (M3)' build` - Build iOS app
- `xcrun simctl install "iPad Air 11-inch (M3)" "./path/to/MagicProxyiOS.app"` - Install on simulator
- `xcrun simctl launch "iPad Air 11-inch (M3)" com.magicproxy.MagicProxyiOS` - Launch iOS app
- `open -a Simulator` - Open iOS Simulator for manual testing
- `cd MagicProxyiOS && open MagicProxyiOS.xcodeproj` - Open in Xcode for development

### Testing and Quality
- `cargo test` - Run all tests
- `cargo clippy` - Run Rust linter
- `cargo fmt` - Format code according to Rust standards

### Development Workflow
- `cargo clean` - Remove build artifacts from target directory
- `cargo update` - Update dependencies in Cargo.lock
- **ALWAYS run `cargo fmt` as the final step after making any code changes** - This ensures consistent code formatting across the entire codebase

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

### Git Workflow (IMPORTANT)

#### Commit Guidelines
- **ALWAYS create a git commit before major refactorings** - This prevents loss of working functionality during complex code restructuring
- Use descriptive commit messages that explain what is working at that state
- Example commit messages: "grid preview working with print selection", "background loading functional", "complete GUI integration"
- Before major architectural changes (moving code between modules, removing/adding large sections), ensure the current state is committed first
- This allows easy recovery if refactoring goes wrong or functionality is accidentally removed

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
- **Location**: `~/.cache/magic-proxy/` (platform-specific cache directory)
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
- `src/app.rs` - Complete Iced application with:
  - **Grid Preview System**: 3x3 preview grids showing actual card images when cached
  - **Print Selection Modal**: Browse alternative printings with thumbnail images
  - **Background Image Loading**: Progressive image loading with rate limiting
  - **Entry-Based Print Selection**: One print choice per decklist entry affects all copies
  - **Page Navigation**: Multi-page navigation for large decklists
  - **Double-Faced Card Support**: Intelligent face detection and mode selection
  - **Expandable Advanced Options Sidebar**: Toggleable sidebar with card name database and image cache management

### CLI Example (`magic-proxy-cli/`)
- `src/main.rs` - Command-line interface demonstrating core functionality

## Usage Examples

### CLI Tool
```bash
# Search for cards
cargo run -p magic-proxy-cli -- search "Lightning Bolt"

# Generate PDF (when implemented)
cargo run -p magic-proxy-cli -- generate --cards="Lightning Bolt,Counterspell" --output=proxies.pdf
```

### Core Library API
```rust
use magic_proxy_core::{ProxyGenerator, PdfOptions, DoubleFaceMode, initialize_caches, shutdown_caches};

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

## Multi-Page Grid Preview with Print Selection

**Status**: ✅ IMPLEMENTED

This feature extends beyond MagicHawk's functionality by providing visual PDF preview with per-card print selection capabilities. Users can now preview exactly what their PDF will look like and select alternative printings for each decklist entry.

### Core Concept

Users can preview exactly what their PDF will look like as 3x3 grids (one per PDF page) and click on any card to select from alternative printings for that decklist entry.

### Key Design Principles

#### Entry-Based Print Selection
- **One selection per decklist entry**: `4x Lightning Bolt` = one print selection affecting all 4 card images
- **Consistent behavior**: All copies of the same decklist entry use the same selected printing
- **Leverages existing logic**: Builds on current `DecklistEntry` structure and set/language parsing

#### Multi-Page Preview System
- **Page-by-page grids**: Each PDF page (9 cards) gets its own 3x3 preview grid
- **Navigation controls**: Previous/Next buttons with "Page X of Y" indicator  
- **Independent selections**: Print choices on different pages are managed separately
- **Persistent state**: Navigate away and back - all selections are maintained

#### Integration with Existing Set Selection
- **Set hints become defaults**: `[LEA]` in decklist makes LEA the initial selection in print picker
- **User override capability**: Any manual selection supersedes the automatic set hint
- **Backward compatibility**: Existing decklist parsing behavior remains unchanged

### Data Structure Design

```rust
/// Multi-page grid preview state
pub struct GridPreview {
    pub entries: Vec<PreviewEntry>,     // One per decklist entry
    pub current_page: usize,            // 0-indexed current page
    pub total_pages: usize,             // Calculated from card count
    pub selected_entry_index: Option<usize>, // For print selection modal
}

/// Represents one decklist entry with all its printings and positions
pub struct PreviewEntry {
    pub decklist_entry: DecklistEntry,     // Original "4x Lightning Bolt [LEA]"
    pub available_printings: Vec<Card>,    // All printings found from search
    pub selected_printing: Option<usize>,  // Index into available_printings
    pub grid_positions: Vec<GridPosition>, // Where this entry's cards appear
}

/// Individual card position in the grid layout
pub struct GridPosition {
    pub page: usize,                    // Which page this position is on
    pub position_in_page: usize,        // 0-8 position within 3x3 grid
    pub entry_index: usize,             // Back-reference to parent entry
    pub copy_number: usize,             // 1st, 2nd, 3rd, 4th copy of entry
}

/// Page navigation state
pub struct PageNavigation {
    pub current_page: usize,            // Current page being viewed
    pub total_pages: usize,             // Total pages calculated from cards
    pub can_go_prev: bool,              // Navigation state
    pub can_go_next: bool,
}
```

### UI/UX Design

#### Grid Preview Interface
- **Visual accuracy**: 3x3 grids show exact PDF page layout
- **Entry grouping**: Visual indicators (borders, badges) show which cards belong to same entry
- **Hover effects**: Highlight all positions of same entry when hovering over any instance
- **Click interaction**: Click any card instance → open print selection for entire entry

#### Print Selection Modal
- **Modal title**: "Select printing for 4x Lightning Bolt [current: LEA]"
- **Thumbnail grid**: Show all available printings as clickable thumbnails
- **Set/language info**: Overlay on each thumbnail showing set code and language
- **Default selection**: Highlight the set hint from decklist (`[LEA]`) if available
- **Immediate update**: Modal closes → all related grid positions update instantly

#### Page Navigation
- **Navigation bar**: "Page 1 of 4" with Previous/Next buttons
- **Page indicators**: Show completion status (e.g., "3 custom selections on this page")
- **Keyboard shortcuts**: Arrow keys for page navigation, ESC to close modals

### New Message Types

```rust
pub enum Message {
    // Existing messages remain unchanged...
    
    // Grid preview lifecycle
    BuildGridPreview,
    GridPreviewBuilt(Result<GridPreview, String>),
    
    // Page navigation
    NextPage,
    PrevPage,
    GoToPage(usize),
    
    // Print selection
    ShowPrintSelection(usize),          // Entry index
    SelectPrint { 
        entry_index: usize, 
        print_index: usize 
    },
    ClosePrintSelection,
    
    // Image loading for preview
    PreviewImageLoaded(String, Vec<u8>), // URL, image data
}
```

### State Integration

The preview system extends the existing `AppState` structure:

```rust
pub struct AppState {
    // Existing fields remain unchanged...
    
    // New preview-related fields
    pub grid_preview: Option<GridPreview>,
    pub page_navigation: Option<PageNavigation>,
    pub preview_mode: PreviewMode,
    pub preview_images: HashMap<String, Vec<u8>>, // Image cache for previews
}

pub enum PreviewMode {
    Hidden,           // Traditional workflow (parse → generate)
    GridPreview,      // Show 3x3 grid preview
    PrintSelection,   // Modal for selecting prints
}
```

### Implementation Status

#### Phase 1: Core Data Structures ✅ COMPLETED
1. ✅ Added preview-related structs to `src/app.rs` (`GridPreview`, `PreviewEntry`, `GridPosition`)
2. ✅ Extended `AppState` with preview fields and `PreviewMode` enum
3. ✅ Implemented grid position calculation logic
4. ✅ Added new message types and handlers

#### Phase 2: Grid Preview UI ✅ COMPLETED  
1. ✅ Created 3x3 grid view with actual card images (no spacing, PDF-accurate)
2. ✅ Implemented page navigation controls with Previous/Next buttons
3. ✅ Added visual card display with fallback to loading text
4. ✅ Handle click events for entry selection and print modal

#### Phase 3: Print Selection Modal ✅ COMPLETED
1. ✅ Created modal overlay showing alternative printings
2. ✅ Implemented 4x4 thumbnail grid for print selection with actual images
3. ✅ Added set/language info overlays on thumbnails
4. ✅ Handle selection and grid update logic

#### Phase 4: Integration & Polish ✅ MOSTLY COMPLETED
1. ✅ Wired into existing decklist parsing workflow
2. 🔄 Keyboard shortcuts and accessibility features (basic implementation)
3. 🔄 Hover effects and visual feedback (future enhancement)
4. ✅ Added loading states and error handling

### Workflow Integration

The feature integrates seamlessly into the existing workflow:

**Current**: `Parse Decklist → Generate PDF → Save`
**Enhanced**: `Parse Decklist → Preview Pages → [Optional: Customize Prints] → Generate PDF → Save`

Users can still use the traditional workflow (skip preview) or take advantage of the enhanced print selection capabilities.

### Technical Benefits

- **Builds on existing architecture**: Leverages `DecklistEntry`, `CardSearchResult`, and image caching
- **Minimal disruption**: Current functionality remains unchanged  
- **Performance optimized**: Reuses cached images and search results
- **Scalable design**: Handles large decklists with efficient pagination
- **User-centric**: Intuitive entry-based grouping matches user mental model

## Recent Architecture Improvements (2025-08-08)

### Single-Source-of-Truth Implementation ✅
Successfully resolved potential inconsistency between grid preview and PDF generation through architectural changes:

#### Key Changes Made:
1. **Evolved `DecklistEntry` Structure**: Replaced `preferred_face: Option<NameMatchMode>` with `face_mode: DoubleFaceMode` for fully resolved face preferences
2. **Updated Core API**: `parse_and_resolve_decklist()` now accepts global face mode parameter and resolves preferences at parse time
3. **Shared Helper Function**: Created `get_image_urls_for_face_mode()` for consistent face mode logic between components
4. **Grid Preview Accuracy**: Updated grid generation to use same logic as PDF generation, ensuring identical results

#### Benefits Achieved:
- **Consistency Guarantee**: Grid preview and PDF generation now show identical results
- **No Timing Issues**: Face preferences resolved once during parsing, immune to subsequent global setting changes  
- **Clean Architecture**: Single implementation of face mode logic eliminates code duplication
- **Risk Mitigation**: PDF generation logic preserved unchanged to avoid regressions
- **Comprehensive Testing**: All face mode combinations verified with unit tests

#### Compatibility:
- **Breaking Change**: `parse_and_resolve_decklist()` signature updated to require global face mode parameter
- **Migration**: GUI updated to pass current face mode setting during parsing
- **Backwards Compatibility**: All other APIs remain unchanged

### Future Enhancements

- **Background Loader Synchronization**: Currently, the background loader caches images for the original printings selected during decklist parsing, but doesn't update when users select different printings in the grid preview. This creates inefficiency where the wrong images are cached while the newly selected printings may need to be fetched later. Solution would involve either canceling and restarting the background loader with updated selections, or implementing a shared state system where both grid preview and background loader reference the same resolved cards.
- **Drag & drop reordering**: Allow users to rearrange card positions within pages
- **Print filtering**: Filter available printings by date, legality, or price
- **Bulk operations**: Select printings for multiple entries at once
- **Export preferences**: Save and reuse print selection preferences
- **Preview export**: Save preview grids as images for sharing

## Recent UI/UX Improvements (2025-08-10)

### Interface Consistency and Layout ✅
Successfully improved the user interface with consistent styling and better information organization:

#### Key UI Changes Made:
1. **Unified Font Sizing**: Added `UI_FONT_SIZE` constant (14pt) applied across all button text and labels
2. **Button Layout Consistency**: All action buttons use consistent width and typography
3. **Expandable Advanced Options Sidebar**: 
   - Toggleable sidebar (480px wide, controlled by `ADVANCED_SIDEBAR_WIDTH` constant)
   - Appears adjacent to main content (no "desert" of empty space)  
   - Toggle button clearly labeled "Advanced Options"
   - Contains visually separated sections for different functionality
4. **Clear Information Architecture**: Moved less frequently needed information to collapsible sidebar
5. **Status Display Repositioning**: Moved status messages below button row for better visual flow

#### Advanced Options Sidebar Design:
- **Card Name Database Section**: Light green background, contains update button and cache statistics
- **Image Cache Section**: Light blue background, shows cache size and image count  
- **Visual Separation**: Each section has distinct styling with subtle borders and tinted backgrounds
- **Consistent Typography**: Section headers at 16pt, details with bullet points at 12pt
- **Compact Layout**: Sidebar positioned directly next to main content, avoiding wasted screen space

#### New Constants Added:
```rust
const UI_FONT_SIZE: u16 = 14;                    // Consistent font sizing
const ADVANCED_SIDEBAR_WIDTH: f32 = 480.0;      // Sidebar width management
```

#### New Message Types:
```rust
pub enum Message {
    // ... existing messages ...
    ToggleExtendedPanel,  // Show/hide advanced options sidebar
}
```

#### AppState Extensions:
```rust
pub struct AppState {
    // ... existing fields ...
    show_extended_panel: bool,  // Track sidebar visibility state
}
```

#### Benefits Achieved:
- **Professional Interface**: Consistent typography and spacing throughout the application
- **Better Information Hierarchy**: Advanced options hidden by default but easily accessible
- **Improved Usability**: Clear button labels and logical grouping of functionality  
- **Maintainable Design**: Global constants make UI tweaking simple and consistent
- **Space Efficiency**: Sidebar appears adjacent to content rather than creating empty screen areas