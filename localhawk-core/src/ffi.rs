use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::OnceLock;

use crate::{
    DoubleFaceMode, PdfOptions, ProxyGenerator,
    background_loading::{BackgroundLoadHandle, LoadingPhase},
    force_update_card_lookup, get_card_names_cache_path, get_card_names_cache_size,
    get_image_cache_info, get_image_cache_path, get_search_cache_path,
    get_search_results_cache_info, initialize_caches, save_caches,
};

/// Global tokio runtime for all FFI operations
static FFI_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get the FFI tokio runtime, returns None if not initialized
fn get_ffi_runtime() -> Option<&'static tokio::runtime::Runtime> {
    FFI_RUNTIME.get()
}

/// Error codes for FFI functions
#[repr(C)]
pub enum FFIError {
    Success = 0,
    NullPointer = -1,
    InvalidInput = -2,
    InitializationFailed = -3,
    ParseFailed = -4,
    PdfGenerationFailed = -5,
    OutOfMemory = -6,
}

/// Initialize the proxy generator caches
/// Must be called before any other FFI functions
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_initialize() -> c_int {
    // Create the global tokio runtime that will be used for all FFI operations
    // Use current_thread runtime to avoid QoS priority inversion on iOS
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return FFIError::InitializationFailed as c_int,
    };

    // Initialize caches using the new runtime
    let init_result = rt.block_on(initialize_caches());

    // Store the runtime globally for all subsequent FFI calls
    if FFI_RUNTIME.set(rt).is_err() {
        // Runtime was already initialized - this is an error
        return FFIError::InitializationFailed as c_int;
    }

    match init_result {
        Ok(_) => FFIError::Success as c_int,
        Err(_) => FFIError::InitializationFailed as c_int,
    }
}

/// Generate PDF from decklist text
///
/// # Arguments
/// * `decklist_cstr` - Null-terminated C string containing the decklist
/// * `output_buffer` - Pointer to buffer pointer (will be allocated by this function)
/// * `output_size` - Pointer to size_t that will receive the buffer size
///
/// # Returns
/// * 0 on success, negative error code on failure
///
/// # Memory Management
/// * The output buffer is allocated by this function using malloc
/// * Caller must call `localhawk_free_buffer` to free the memory
/// * If function fails, no memory is allocated
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_generate_pdf_from_decklist(
    decklist_cstr: *const c_char,
    output_buffer: *mut *mut u8,
    output_size: *mut usize,
) -> c_int {
    // Validate input parameters
    if decklist_cstr.is_null() || output_buffer.is_null() || output_size.is_null() {
        return FFIError::NullPointer as c_int;
    }

    // Convert C string to Rust string
    let decklist_text = match unsafe { CStr::from_ptr(decklist_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    if decklist_text.trim().is_empty() {
        return FFIError::InvalidInput as c_int;
    }

    // Set up tokio runtime for async operations
    // For iOS: Use current_thread to inherit caller's QoS and avoid priority inversion
    // This means the tokio runtime runs on the same thread as the caller,
    // inheriting whatever QoS the Swift code chose
    //
    // TRADE-OFFS:
    // ✅ Eliminates QoS priority inversion warnings
    // ✅ Simple threading model, easier to debug
    // ✅ Inherits caller's QoS automatically
    // ❌ Limits parallelism (e.g., image downloads are sequential)
    // ❌ May be slower for highly parallel workloads
    //
    // TODO: Consider multi-threaded runtime with QoS matching for better parallelism:
    // - Image downloads could be parallel (not rate-limited by Scryfall)
    // - PDF generation steps could overlap
    // - Would require careful QoS thread configuration
    //
    // Future multi-threaded approach might look like:
    // ```rust
    // let rt = tokio::runtime::Builder::new_multi_thread()
    //     .worker_threads(2) // Limit for mobile
    //     .thread_name("localhawk-worker")
    //     .on_thread_start(|| {
    //         // Set QoS to match caller's thread on iOS
    //         #[cfg(target_os = "ios")]
    //         unsafe {
    //             // Would need iOS-specific QoS inheritance code
    //         }
    //     })
    //     .build()?;
    // ```
    
    // Use the global runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    // Generate PDF using existing core functionality
    let pdf_data = match rt.block_on(async {
        // Parse and resolve decklist entries
        let entries = ProxyGenerator::parse_and_resolve_decklist(
            decklist_text,
            DoubleFaceMode::BothSides, // Default for mobile - show both faces
        )
        .await?;

        if entries.is_empty() {
            return Err(crate::ProxyError::InvalidCard(
                "No valid cards found in decklist".to_string(),
            ));
        }

        // Generate PDF with default options
        let pdf_options = PdfOptions::default();
        ProxyGenerator::generate_pdf_from_entries(&entries, pdf_options, |current, total| {
            // Simple progress callback - could be enhanced later
            log::debug!("PDF generation progress: {}/{}", current, total);
        })
        .await
    }) {
        Ok(data) => data,
        Err(e) => {
            log::error!("PDF generation failed: {:?}", e);
            return match e {
                crate::ProxyError::InvalidCard(_) => FFIError::ParseFailed as c_int,
                _ => FFIError::PdfGenerationFailed as c_int,
            };
        }
    };

    // Allocate buffer for the PDF data
    let buffer_size = pdf_data.len();
    let buffer = unsafe {
        let ptr = libc::malloc(buffer_size) as *mut u8;
        if ptr.is_null() {
            return FFIError::OutOfMemory as c_int;
        }

        // Copy PDF data to allocated buffer
        std::ptr::copy_nonoverlapping(pdf_data.as_ptr(), ptr, buffer_size);
        ptr
    };

    // Set output parameters
    unsafe {
        *output_buffer = buffer;
        *output_size = buffer_size;
    }

    FFIError::Success as c_int
}

/// Free buffer allocated by localhawk_generate_pdf_from_decklist
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_free_buffer(buffer: *mut u8) {
    if !buffer.is_null() {
        unsafe {
            libc::free(buffer as *mut libc::c_void);
        }
    }
}

/// Get error message for the last error (simple version)
/// Returns a static string describing the error code
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_error_message(error_code: c_int) -> *const c_char {
    let message = match error_code {
        x if x == FFIError::Success as c_int => "Success",
        x if x == FFIError::NullPointer as c_int => "Null pointer argument",
        x if x == FFIError::InvalidInput as c_int => "Invalid input string",
        x if x == FFIError::InitializationFailed as c_int => "Failed to initialize caches",
        x if x == FFIError::ParseFailed as c_int => {
            "Failed to parse decklist or no valid cards found"
        }
        x if x == FFIError::PdfGenerationFailed as c_int => "Failed to generate PDF",
        x if x == FFIError::OutOfMemory as c_int => "Out of memory",
        _ => "Unknown error",
    };

    // Return pointer to static string (no need to free)
    message.as_ptr() as *const c_char
}

/// Simple test function to verify FFI is working
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_test_connection() -> c_int {
    42 // Magic number to verify the call works
}

/// Cache statistics structure for FFI
#[repr(C)]
pub struct CacheStats {
    pub count: u32,
    pub size_mb: f64,
}

/// Get image cache statistics
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_image_cache_stats() -> CacheStats {
    let (count, size_mb) = get_image_cache_info();
    CacheStats {
        count: count as u32,
        size_mb,
    }
}

/// Get search results cache statistics
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_search_cache_stats() -> CacheStats {
    let (count, size_mb) = get_search_results_cache_info();
    CacheStats {
        count: count as u32,
        size_mb,
    }
}

/// Get card names cache statistics
/// Returns count = 0 if cache is not initialized
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_card_names_cache_stats() -> CacheStats {
    if let Some((count, size_mb)) = get_card_names_cache_size() {
        CacheStats {
            count: count as u32,
            size_mb,
        }
    } else {
        CacheStats {
            count: 0,
            size_mb: 0.0,
        }
    }
}

/// Clear image cache
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_clear_image_cache() -> c_int {
    match ProxyGenerator::clear_cache() {
        Ok(_) => FFIError::Success as c_int,
        Err(_) => FFIError::InitializationFailed as c_int,
    }
}

/// Update card names database from Scryfall API
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_update_card_names() -> c_int {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return FFIError::InitializationFailed as c_int,
    };

    match rt.block_on(force_update_card_lookup()) {
        Ok(_) => FFIError::Success as c_int,
        Err(_) => FFIError::InitializationFailed as c_int,
    }
}

/// Save all in-memory caches to disk
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_save_caches() -> c_int {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return FFIError::InitializationFailed as c_int,
    };

    match rt.block_on(save_caches()) {
        Ok(_) => FFIError::Success as c_int,
        Err(_) => FFIError::InitializationFailed as c_int,
    }
}

/// Get image cache path
/// Returns a newly allocated C string that must be freed with localhawk_free_string
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_image_cache_path() -> *mut c_char {
    let path = get_image_cache_path();
    match CString::new(path) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get search cache path
/// Returns a newly allocated C string that must be freed with localhawk_free_string
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_search_cache_path() -> *mut c_char {
    let path = get_search_cache_path();
    match CString::new(path) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get card names cache path
/// Returns a newly allocated C string that must be freed with localhawk_free_string
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_card_names_cache_path() -> *mut c_char {
    let path = get_card_names_cache_path();
    match CString::new(path) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Free a string allocated by localhawk_get_*_path functions
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
            // CString will be dropped and memory freed automatically
        }
    }
}

// ============================================================================
// Print Selection & Preview FFI Extensions
// ============================================================================

/// C-compatible decklist entry structure
#[repr(C)]
pub struct CDeclistEntry {
    pub multiple: i32,
    pub name: *mut c_char,
    pub set: *mut c_char,        // NULL if not specified
    pub language: *mut c_char,   // NULL if not specified
    pub face_mode: c_int,        // DoubleFaceMode as int
    pub source_line_number: i32, // -1 if not specified
}

/// C-compatible card printing structure
#[repr(C)]
pub struct CCardPrinting {
    pub name: *mut c_char,
    pub set: *mut c_char,
    pub language: *mut c_char,
    pub border_crop: *mut c_char,
    pub back_side: *mut c_char, // NULL if no back side
}

/// C-compatible card search result
#[repr(C)]
pub struct CCardSearchResult {
    pub cards: *mut CCardPrinting,
    pub count: usize,
}

/// Parse decklist and return resolved entries
/// Returns an array of CDeclistEntry structures
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_parse_and_resolve_decklist(
    decklist_cstr: *const c_char,
    global_face_mode: c_int,
    output_entries: *mut *mut CDeclistEntry,
    output_count: *mut usize,
) -> c_int {
    if decklist_cstr.is_null() || output_entries.is_null() || output_count.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let decklist_text = match unsafe { CStr::from_ptr(decklist_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    let face_mode = match global_face_mode {
        0 => DoubleFaceMode::FrontOnly,
        1 => DoubleFaceMode::BackOnly,
        2 => DoubleFaceMode::BothSides,
        _ => return FFIError::InvalidInput as c_int,
    };

    // Use the FFI runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    let entries = match rt.block_on(async {
        ProxyGenerator::parse_and_resolve_decklist(decklist_text, face_mode).await
    }) {
        Ok(entries) => entries,
        Err(_) => return FFIError::ParseFailed as c_int,
    };

    // Convert to C structures
    let mut c_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = match CString::new(entry.name) {
            Ok(s) => s.into_raw(),
            Err(_) => return FFIError::OutOfMemory as c_int,
        };
        let set = entry
            .set
            .map(|s| CString::new(s).ok())
            .flatten()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
        let language = entry
            .lang
            .map(|s| CString::new(s).ok())
            .flatten()
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
        let face_mode_int = match entry.face_mode {
            DoubleFaceMode::FrontOnly => 0,
            DoubleFaceMode::BackOnly => 1,
            DoubleFaceMode::BothSides => 2,
        };

        c_entries.push(CDeclistEntry {
            multiple: entry.multiple,
            name,
            set,
            language,
            face_mode: face_mode_int,
            source_line_number: entry.source_line_number.map(|n| n as i32).unwrap_or(-1),
        });
    }

    // Allocate array
    let array_size = c_entries.len() * std::mem::size_of::<CDeclistEntry>();
    let array_ptr = unsafe { libc::malloc(array_size) as *mut CDeclistEntry };
    if array_ptr.is_null() {
        // Clean up allocated strings
        for entry in c_entries {
            if !entry.name.is_null() {
                unsafe {
                    let _ = CString::from_raw(entry.name);
                }
            }
            if !entry.set.is_null() {
                unsafe {
                    let _ = CString::from_raw(entry.set);
                }
            }
            if !entry.language.is_null() {
                unsafe {
                    let _ = CString::from_raw(entry.language);
                }
            }
        }
        return FFIError::OutOfMemory as c_int;
    }

    // Copy entries to array
    unsafe {
        std::ptr::copy_nonoverlapping(c_entries.as_ptr(), array_ptr, c_entries.len());
        *output_entries = array_ptr;
        *output_count = c_entries.len();
    }

    FFIError::Success as c_int
}

/// Search for all printings of a specific card
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_search_card_printings(
    card_name_cstr: *const c_char,
    output_result: *mut *mut CCardSearchResult,
) -> c_int {
    if card_name_cstr.is_null() || output_result.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let card_name = match unsafe { CStr::from_ptr(card_name_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    // Use the FFI runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    let search_result = match rt.block_on(async { ProxyGenerator::search_card(card_name).await }) {
        Ok(result) => result,
        Err(_) => return FFIError::ParseFailed as c_int,
    };

    // Convert cards to C structures
    let mut c_cards = Vec::with_capacity(search_result.cards.len());
    for card in search_result.cards {
        let name = match CString::new(card.name) {
            Ok(s) => s.into_raw(),
            Err(_) => return FFIError::OutOfMemory as c_int,
        };
        let set = match CString::new(card.set) {
            Ok(s) => s.into_raw(),
            Err(_) => return FFIError::OutOfMemory as c_int,
        };
        let language = match CString::new(card.language) {
            Ok(s) => s.into_raw(),
            Err(_) => return FFIError::OutOfMemory as c_int,
        };
        let border_crop = match CString::new(card.border_crop) {
            Ok(s) => s.into_raw(),
            Err(_) => return FFIError::OutOfMemory as c_int,
        };
        let back_side = match card.back_side {
            Some(back) => {
                // Extract image URL from BackSide enum
                let url = match back {
                    crate::scryfall::models::BackSide::DfcBack { image_url, .. } => image_url,
                    crate::scryfall::models::BackSide::ContributesToMeld {
                        meld_result_image_url,
                        ..
                    } => meld_result_image_url,
                };
                match CString::new(url) {
                    Ok(s) => s.into_raw(),
                    Err(_) => return FFIError::OutOfMemory as c_int,
                }
            }
            None => std::ptr::null_mut(),
        };

        c_cards.push(CCardPrinting {
            name,
            set,
            language,
            border_crop,
            back_side,
        });
    }

    // Allocate array and result structure
    let cards_array_size = c_cards.len() * std::mem::size_of::<CCardPrinting>();
    let cards_ptr = unsafe { libc::malloc(cards_array_size) as *mut CCardPrinting };
    let result_ptr =
        unsafe { libc::malloc(std::mem::size_of::<CCardSearchResult>()) as *mut CCardSearchResult };

    if cards_ptr.is_null() || result_ptr.is_null() {
        // Clean up on failure
        for card in c_cards {
            unsafe {
                if !card.name.is_null() {
                    let _ = CString::from_raw(card.name);
                }
                if !card.set.is_null() {
                    let _ = CString::from_raw(card.set);
                }
                if !card.language.is_null() {
                    let _ = CString::from_raw(card.language);
                }
                if !card.border_crop.is_null() {
                    let _ = CString::from_raw(card.border_crop);
                }
                if !card.back_side.is_null() {
                    let _ = CString::from_raw(card.back_side);
                }
            }
        }
        if !cards_ptr.is_null() {
            unsafe {
                libc::free(cards_ptr as *mut libc::c_void);
            }
        }
        if !result_ptr.is_null() {
            unsafe {
                libc::free(result_ptr as *mut libc::c_void);
            }
        }
        return FFIError::OutOfMemory as c_int;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(c_cards.as_ptr(), cards_ptr, c_cards.len());
        *result_ptr = CCardSearchResult {
            cards: cards_ptr,
            count: c_cards.len(),
        };
        *output_result = result_ptr;
    }

    FFIError::Success as c_int
}

/// Free decklist entries array allocated by localhawk_parse_and_resolve_decklist
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_free_decklist_entries(entries: *mut CDeclistEntry, count: usize) {
    if !entries.is_null() {
        unsafe {
            for i in 0..count {
                let entry = entries.add(i);
                if !(*entry).name.is_null() {
                    let _ = CString::from_raw((*entry).name);
                }
                if !(*entry).set.is_null() {
                    let _ = CString::from_raw((*entry).set);
                }
                if !(*entry).language.is_null() {
                    let _ = CString::from_raw((*entry).language);
                }
            }
            libc::free(entries as *mut libc::c_void);
        }
    }
}

/// Free card search result allocated by localhawk_search_card_printings
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_free_card_search_result(result: *mut CCardSearchResult) {
    if !result.is_null() {
        unsafe {
            let result_ref = &*result;
            if !result_ref.cards.is_null() {
                for i in 0..result_ref.count {
                    let card = result_ref.cards.add(i);
                    if !(*card).name.is_null() {
                        let _ = CString::from_raw((*card).name);
                    }
                    if !(*card).set.is_null() {
                        let _ = CString::from_raw((*card).set);
                    }
                    if !(*card).language.is_null() {
                        let _ = CString::from_raw((*card).language);
                    }
                    if !(*card).border_crop.is_null() {
                        let _ = CString::from_raw((*card).border_crop);
                    }
                    if !(*card).back_side.is_null() {
                        let _ = CString::from_raw((*card).back_side);
                    }
                }
                libc::free(result_ref.cards as *mut libc::c_void);
            }
            libc::free(result as *mut libc::c_void);
        }
    }
}

/// Get cached image bytes for a given URL
/// Returns image data if cached, NULL if not cached
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_cached_image_bytes(
    image_url_cstr: *const c_char,
    output_buffer: *mut *mut u8,
    output_size: *mut usize,
) -> c_int {
    if image_url_cstr.is_null() || output_buffer.is_null() || output_size.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let image_url = match unsafe { CStr::from_ptr(image_url_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    match crate::get_cached_image_bytes(image_url) {
        Some(bytes) => {
            let buffer = unsafe { libc::malloc(bytes.len()) as *mut u8 };
            if buffer.is_null() {
                return FFIError::OutOfMemory as c_int;
            }

            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
                *output_buffer = buffer;
                *output_size = bytes.len();
            }

            FFIError::Success as c_int
        }
        None => {
            unsafe {
                *output_buffer = std::ptr::null_mut();
                *output_size = 0;
            }
            FFIError::ParseFailed as c_int // Using ParseFailed to indicate "not found"
        }
    }
}

/// Check if an image is cached without retrieving the bytes
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_is_image_cached(image_url_cstr: *const c_char) -> c_int {
    if image_url_cstr.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let image_url = match unsafe { CStr::from_ptr(image_url_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    if crate::get_cached_image_bytes(image_url).is_some() {
        1 // TRUE - image is cached
    } else {
        0 // FALSE - image is not cached
    }
}

// ============================================================================
// Background Loading FFI Extensions
// ============================================================================

/// Parse decklist and start background image loading (fire and forget)
/// This function parses the decklist, starts background loading, and returns immediately
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_parse_and_start_background_loading(
    decklist_cstr: *const c_char,
    global_face_mode: c_int,
) -> c_int {
    if decklist_cstr.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let decklist = match unsafe { CStr::from_ptr(decklist_cstr) }.to_str() {
        Ok(s) => s,
        Err(_) => return FFIError::InvalidInput as c_int,
    };

    let face_mode = match global_face_mode {
        0 => DoubleFaceMode::FrontOnly,
        1 => DoubleFaceMode::BackOnly,
        2 => DoubleFaceMode::BothSides,
        _ => DoubleFaceMode::BothSides,
    };

    // Use the global runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    // Use the persistent runtime - background tasks will continue after this function returns
    match rt.block_on(async {
        // Parse the decklist first
        let entries = crate::ProxyGenerator::parse_and_resolve_decklist(decklist, face_mode).await?;
        
        // Start background loading for all entries (fire and forget)
        if !entries.is_empty() {
            let entries_clone = entries.clone();
            let entry_count = entries.len();
            println!("About to spawn background loading task for {} entries", entry_count);
            
            // Spawn the task and give it a moment to start
            let handle = tokio::spawn(async move {
                println!("Background loading task started for {} entries", entry_count);
                let _handle = crate::start_background_image_loading(entries_clone);
                println!("Background loading task completed for {} entries", entry_count);
            });
            
            // Give the spawned task a moment to start before returning
            // This ensures the task actually begins execution
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            println!("tokio::spawn called successfully, gave task time to start");
        } else {
            println!("No entries to load in background");
        }
        
        Ok::<Vec<crate::DecklistEntry>, crate::ProxyError>(entries)
    }) {
        Ok(_entries) => FFIError::Success as c_int,
        Err(_) => FFIError::ParseFailed as c_int,
    }
}

/// C-compatible loading phase enum
#[repr(C)]
pub enum CLoadingPhase {
    Selected = 0,     // Loading selected printings (based on set/lang hints)
    Alternatives = 1, // Loading alternative printings
    Completed = 2,    // All done
}

/// C-compatible background load progress structure
#[repr(C)]
pub struct CBackgroundLoadProgress {
    pub phase: CLoadingPhase,
    pub current_entry: usize,
    pub total_entries: usize,
    pub selected_loaded: usize,
    pub alternatives_loaded: usize,
    pub total_alternatives: usize,
    pub error_count: usize,
}

/// Opaque handle to background loading task
pub type BackgroundLoadHandleId = usize;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    static ref BACKGROUND_HANDLES: Arc<Mutex<HashMap<BackgroundLoadHandleId, BackgroundLoadHandle>>> =
        Arc::new(Mutex::new(HashMap::new()));
    static ref HANDLE_COUNTER: Arc<Mutex<BackgroundLoadHandleId>> = Arc::new(Mutex::new(0));
}

/// Start background image loading for decklist entries
/// Returns a handle ID for tracking progress and cancellation
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_start_background_loading(
    entries: *const CDeclistEntry,
    count: usize,
    handle_id: *mut BackgroundLoadHandleId,
) -> c_int {
    if entries.is_null() || handle_id.is_null() {
        return FFIError::NullPointer as c_int;
    }

    if count == 0 {
        return FFIError::InvalidInput as c_int;
    }

    // Convert C entries to Rust entries
    let rust_entries = unsafe {
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let entry = entries.add(i);
            let name = CStr::from_ptr((*entry).name).to_string_lossy().to_string();
            let set = if (*entry).set.is_null() {
                None
            } else {
                Some(CStr::from_ptr((*entry).set).to_string_lossy().to_string())
            };
            let language = if (*entry).language.is_null() {
                None
            } else {
                Some(
                    CStr::from_ptr((*entry).language)
                        .to_string_lossy()
                        .to_string(),
                )
            };
            let face_mode = match (*entry).face_mode {
                0 => DoubleFaceMode::FrontOnly,
                1 => DoubleFaceMode::BackOnly,
                2 => DoubleFaceMode::BothSides,
                _ => DoubleFaceMode::BothSides, // Default fallback
            };
            let source_line_number = if (*entry).source_line_number >= 0 {
                Some((*entry).source_line_number as usize)
            } else {
                None
            };

            result.push(crate::DecklistEntry {
                multiple: (*entry).multiple,
                name,
                set,
                lang: language,
                face_mode,
                source_line_number,
            });
        }
        result
    };

    // Use the FFI runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    // Convert to a decklist string for the high-level function
    let decklist_string = rust_entries
        .iter()
        .map(|entry| {
            let mut line = format!("{} {}", entry.multiple, entry.name);
            if let Some(ref set) = entry.set {
                line.push_str(&format!(" [{}]", set));
            }
            if let Some(ref lang) = entry.lang {
                line.push_str(&format!(" [{}]", lang));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Use the new high-level function that parses and starts background loading
    let global_face_mode = match rust_entries.first() {
        Some(entry) => entry.face_mode.clone(),
        None => crate::DoubleFaceMode::BothSides,
    };

    let _entries = match rt.block_on(crate::ProxyGenerator::parse_and_start_background_loading(
        &decklist_string,
        global_face_mode,
    )) {
        Ok(entries) => entries,
        Err(_) => return FFIError::ParseFailed as c_int,
    };

    // Since we're using fire-and-forget, just return a dummy handle ID
    unsafe {
        *handle_id = 1; // Always return 1 since we don't track handles anymore
    }

    FFIError::Success as c_int
}

/// Get progress for a background loading task
/// Returns latest progress if available, or indicates if task is finished
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_get_background_progress(
    handle_id: BackgroundLoadHandleId,
    progress: *mut CBackgroundLoadProgress,
    has_progress: *mut c_int,
) -> c_int {
    if progress.is_null() || has_progress.is_null() {
        return FFIError::NullPointer as c_int;
    }

    let mut handles = BACKGROUND_HANDLES.lock().unwrap();
    if let Some(handle) = handles.get_mut(&handle_id) {
        if let Some(rust_progress) = handle.try_get_progress() {
            unsafe {
                *progress = CBackgroundLoadProgress {
                    phase: match rust_progress.phase {
                        LoadingPhase::Selected => CLoadingPhase::Selected,
                        LoadingPhase::Alternatives => CLoadingPhase::Alternatives,
                        LoadingPhase::Completed => CLoadingPhase::Completed,
                    },
                    current_entry: rust_progress.current_entry,
                    total_entries: rust_progress.total_entries,
                    selected_loaded: rust_progress.selected_loaded,
                    alternatives_loaded: rust_progress.alternatives_loaded,
                    total_alternatives: rust_progress.total_alternatives,
                    error_count: rust_progress.errors.len(),
                };
                *has_progress = 1; // TRUE
            }
        } else {
            unsafe {
                *has_progress = 0; // FALSE - no new progress
            }
        }

        // Clean up finished tasks
        if handle.is_finished() {
            handles.remove(&handle_id);
        }

        FFIError::Success as c_int
    } else {
        // Handle not found (probably finished and cleaned up)
        unsafe {
            *has_progress = 0;
        }
        FFIError::InvalidInput as c_int
    }
}

/// Cancel background loading task
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_cancel_background_loading(handle_id: BackgroundLoadHandleId) -> c_int {
    let mut handles = BACKGROUND_HANDLES.lock().unwrap();
    if let Some(handle) = handles.get(&handle_id) {
        handle.cancel();
        handles.remove(&handle_id); // Remove immediately after cancellation
        FFIError::Success as c_int
    } else {
        FFIError::InvalidInput as c_int
    }
}

/// Check if background loading task is finished
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_is_background_loading_finished(
    handle_id: BackgroundLoadHandleId,
) -> c_int {
    let mut handles = BACKGROUND_HANDLES.lock().unwrap();
    if let Some(handle) = handles.get(&handle_id) {
        if handle.is_finished() {
            handles.remove(&handle_id); // Clean up finished task
            1 // TRUE - finished
        } else {
            0 // FALSE - still running
        }
    } else {
        1 // TRUE - not found means it's finished (and cleaned up)
    }
}

/// Generate PDF from an array of DecklistEntry structures
/// This allows PDF generation with modified entries (e.g., after print selection)
#[unsafe(no_mangle)]
pub extern "C" fn localhawk_generate_pdf_from_entries(
    entries: *const CDeclistEntry,
    entry_count: usize,
    output_buffer: *mut *mut u8,
    output_size: *mut usize,
) -> c_int {
    if entries.is_null() || output_buffer.is_null() || output_size.is_null() {
        return FFIError::NullPointer as c_int;
    }

    if entry_count == 0 {
        return FFIError::InvalidInput as c_int;
    }

    // Convert C structures to Rust DecklistEntry structures
    let mut rust_entries = Vec::new();
    for i in 0..entry_count {
        let c_entry = unsafe { &*entries.add(i) };

        // Convert C strings to Rust strings
        let name = if c_entry.name.is_null() {
            return FFIError::InvalidInput as c_int;
        } else {
            match unsafe { CStr::from_ptr(c_entry.name) }.to_str() {
                Ok(s) => s.to_string(),
                Err(_) => return FFIError::InvalidInput as c_int,
            }
        };

        let set = if c_entry.set.is_null() {
            None
        } else {
            match unsafe { CStr::from_ptr(c_entry.set) }.to_str() {
                Ok(s) => Some(s.to_string()),
                Err(_) => return FFIError::InvalidInput as c_int,
            }
        };

        let lang = if c_entry.language.is_null() {
            None
        } else {
            match unsafe { CStr::from_ptr(c_entry.language) }.to_str() {
                Ok(s) => Some(s.to_string()),
                Err(_) => return FFIError::InvalidInput as c_int,
            }
        };

        let face_mode = match c_entry.face_mode {
            0 => DoubleFaceMode::FrontOnly,
            1 => DoubleFaceMode::BackOnly,
            2 => DoubleFaceMode::BothSides,
            _ => DoubleFaceMode::BothSides,
        };

        let source_line_number = if c_entry.source_line_number < 0 {
            None
        } else {
            Some(c_entry.source_line_number as usize)
        };

        rust_entries.push(crate::DecklistEntry {
            multiple: c_entry.multiple,
            name,
            set,
            lang,
            face_mode,
            source_line_number,
        });
    }

    // Use the FFI runtime instead of creating a temporary one
    let rt = match get_ffi_runtime() {
        Some(rt) => rt,
        None => return FFIError::InitializationFailed as c_int, // Must call localhawk_initialize first
    };

    // Generate PDF using existing core functionality
    let pdf_data = match rt.block_on(async {
        let pdf_options = PdfOptions::default();
        ProxyGenerator::generate_pdf_from_entries(&rust_entries, pdf_options, |current, total| {
            log::debug!("PDF generation progress: {}/{}", current, total);
        })
        .await
    }) {
        Ok(data) => data,
        Err(_) => return FFIError::PdfGenerationFailed as c_int,
    };

    // Allocate buffer for PDF data
    let buffer = unsafe { libc::malloc(pdf_data.len()) as *mut u8 };
    if buffer.is_null() {
        return FFIError::OutOfMemory as c_int;
    }

    // Copy PDF data to buffer
    unsafe {
        std::ptr::copy_nonoverlapping(pdf_data.as_ptr(), buffer, pdf_data.len());
        *output_buffer = buffer;
        *output_size = pdf_data.len();
    }

    FFIError::Success as c_int
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_ffi_basic_flow() {
        // Test initialization
        let _init_result = localhawk_initialize();
        // Note: This will likely fail in tests without proper setup, but shouldn't crash

        // Test connection
        assert_eq!(localhawk_test_connection(), 42);

        // Test error message function
        let msg_ptr = localhawk_get_error_message(FFIError::InvalidInput as c_int);
        assert!(!msg_ptr.is_null());
    }

    #[test]
    fn test_null_pointer_handling() {
        let mut buffer: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;

        // Test with null decklist
        let result = localhawk_generate_pdf_from_decklist(ptr::null(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::NullPointer as c_int);

        // Test with null output buffer pointer
        let test_str = CString::new("1 Lightning Bolt").unwrap();
        let result =
            localhawk_generate_pdf_from_decklist(test_str.as_ptr(), ptr::null_mut(), &mut size);
        assert_eq!(result, FFIError::NullPointer as c_int);
    }

    #[test]
    fn test_empty_input_handling() {
        let mut buffer: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;

        // Test with empty string
        let test_str = CString::new("").unwrap();
        let result =
            localhawk_generate_pdf_from_decklist(test_str.as_ptr(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::InvalidInput as c_int);

        // Test with whitespace-only string
        let test_str = CString::new("   \n  \t  ").unwrap();
        let result =
            localhawk_generate_pdf_from_decklist(test_str.as_ptr(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::InvalidInput as c_int);
    }
}
