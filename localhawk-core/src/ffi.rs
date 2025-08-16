use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use crate::{
    DoubleFaceMode, PdfOptions, ProxyGenerator, force_update_card_lookup,
    get_card_names_cache_path, get_card_names_cache_size, get_image_cache_info,
    get_image_cache_path, get_search_cache_path, get_search_results_cache_info, initialize_caches,
    save_caches,
};

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
    // Set up a basic tokio runtime for the blocking call
    // Use current_thread runtime to avoid QoS priority inversion on iOS
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build() {
        Ok(rt) => rt,
        Err(_) => return FFIError::InitializationFailed as c_int,
    };

    match rt.block_on(initialize_caches()) {
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
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build() {
        Ok(rt) => rt,
        Err(_) => return FFIError::InitializationFailed as c_int,
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
