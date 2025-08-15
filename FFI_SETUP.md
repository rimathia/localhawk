# Magic Proxy iOS FFI Setup

## Overview
This document describes the Rust FFI (Foreign Function Interface) layer created for building a native iOS app with SwiftUI.

## What We've Built

### 1. Rust FFI Layer (`magic-proxy-core/src/ffi.rs`)
- **Simple API**: One main function to generate PDF from decklist text
- **Memory Safe**: Proper C-style memory management with explicit free functions
- **Error Handling**: Clear error codes and messages
- **Thread Safe**: Uses tokio runtime for async operations

### 2. C Header File (`magic-proxy-core/include/magic_proxy.h`)
- C-compatible function declarations
- Error code definitions
- Documentation for Swift bridging

### 3. Build System
- `build_ios.sh`: Script to build universal iOS libraries
- Support for both device (ARM64) and simulator (x86_64 + ARM64)
- Test scripts to verify functionality

## Core FFI Functions

### `proxy_initialize()`
- Must be called first to set up caches
- Returns error code (0 = success)

### `proxy_generate_pdf_from_decklist()`
- Takes: decklist text (C string)
- Returns: PDF data buffer + size
- Caller must free buffer with `proxy_free_buffer()`

### `proxy_free_buffer()`
- Frees memory allocated by PDF generation
- Essential for preventing memory leaks

### `proxy_test_connection()`
- Simple function that returns 42
- Useful for verifying FFI is working

## Memory Management
- **Rust allocates** PDF data using `malloc`
- **Swift must free** using `proxy_free_buffer()`
- **No memory leaks** if used correctly

## Usage Pattern
```c
// 1. Initialize
int result = proxy_initialize();
if (result != 0) { /* handle error */ }

// 2. Generate PDF  
uint8_t* pdf_buffer = NULL;
size_t pdf_size = 0;
result = proxy_generate_pdf_from_decklist(
    "1 Lightning Bolt\n1 Counterspell", 
    &pdf_buffer, 
    &pdf_size
);

if (result == 0) {
    // Success - use pdf_buffer and pdf_size
    // ...
    
    // 3. Free memory
    proxy_free_buffer(pdf_buffer);
}
```

## Next Steps for iOS App

### 1. Build iOS Libraries
```bash
./build_ios.sh
```
This creates:
- `ios-libs/libmagic_proxy_core_device.a` (for physical devices)
- `ios-libs/libmagic_proxy_core_sim.a` (for simulator)
- `ios-libs/magic_proxy.h` (header file)

### 2. Xcode Project Setup
1. Create new iOS app project
2. Add appropriate `.a` file to project
3. Add `magic_proxy.h` to bridging header
4. Import in Swift code

### 3. Swift Wrapper Example
```swift
class ProxyGenerator {
    static func generatePDF(from decklist: String) -> Data? {
        // Initialize if needed
        let initResult = proxy_initialize()
        guard initResult == 0 else { return nil }
        
        // Generate PDF
        var buffer: UnsafeMutablePointer<UInt8>?
        var size: Int = 0
        
        let result = proxy_generate_pdf_from_decklist(
            decklist.cString(using: .utf8),
            &buffer,
            &size
        )
        
        guard result == 0, let buffer = buffer else { return nil }
        
        // Create Data and free buffer
        let data = Data(bytes: buffer, count: size)
        proxy_free_buffer(buffer)
        
        return data
    }
}
```

## Testing

### Run Rust FFI tests:
```bash
cargo test -p magic-proxy-core ffi
```

### Test C integration (macOS only):
```bash
./test_ffi_build.sh
```

## Current Status
✅ FFI layer implemented and tested  
✅ C header created  
✅ Build scripts ready  
⏳ iOS project not yet created  
⏳ Swift integration not yet implemented  

## Architecture Benefits
- **Reuses all existing logic**: Card parsing, API calls, PDF generation
- **Minimal FFI surface**: Just 4 functions to maintain
- **Memory safe**: Clear ownership and cleanup rules
- **Cross-platform**: Same core logic works on desktop and mobile