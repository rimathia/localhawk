use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

use crate::{DoubleFaceMode, PdfOptions, ProxyGenerator, initialize_caches};

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
pub extern "C" fn proxy_initialize() -> c_int {
    // Set up a basic tokio runtime for the blocking call
    let rt = match tokio::runtime::Runtime::new() {
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
/// * Caller must call `proxy_free_buffer` to free the memory
/// * If function fails, no memory is allocated
#[unsafe(no_mangle)]
pub extern "C" fn proxy_generate_pdf_from_decklist(
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
    let rt = match tokio::runtime::Runtime::new() {
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

/// Free buffer allocated by proxy_generate_pdf_from_decklist
#[unsafe(no_mangle)]
pub extern "C" fn proxy_free_buffer(buffer: *mut u8) {
    if !buffer.is_null() {
        unsafe {
            libc::free(buffer as *mut libc::c_void);
        }
    }
}

/// Get error message for the last error (simple version)
/// Returns a static string describing the error code
#[unsafe(no_mangle)]
pub extern "C" fn proxy_get_error_message(error_code: c_int) -> *const c_char {
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
pub extern "C" fn proxy_test_connection() -> c_int {
    42 // Magic number to verify the call works
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::ptr;

    #[test]
    fn test_ffi_basic_flow() {
        // Test initialization
        let init_result = proxy_initialize();
        // Note: This will likely fail in tests without proper setup, but shouldn't crash

        // Test connection
        assert_eq!(proxy_test_connection(), 42);

        // Test error message function
        let msg_ptr = proxy_get_error_message(FFIError::InvalidInput as c_int);
        assert!(!msg_ptr.is_null());
    }

    #[test]
    fn test_null_pointer_handling() {
        let mut buffer: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;

        // Test with null decklist
        let result = proxy_generate_pdf_from_decklist(ptr::null(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::NullPointer as c_int);

        // Test with null output buffer pointer
        let test_str = CString::new("1 Lightning Bolt").unwrap();
        let result =
            proxy_generate_pdf_from_decklist(test_str.as_ptr(), ptr::null_mut(), &mut size);
        assert_eq!(result, FFIError::NullPointer as c_int);
    }

    #[test]
    fn test_empty_input_handling() {
        let mut buffer: *mut u8 = ptr::null_mut();
        let mut size: usize = 0;

        // Test with empty string
        let test_str = CString::new("").unwrap();
        let result = proxy_generate_pdf_from_decklist(test_str.as_ptr(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::InvalidInput as c_int);

        // Test with whitespace-only string
        let test_str = CString::new("   \n  \t  ").unwrap();
        let result = proxy_generate_pdf_from_decklist(test_str.as_ptr(), &mut buffer, &mut size);
        assert_eq!(result, FFIError::InvalidInput as c_int);
    }
}
