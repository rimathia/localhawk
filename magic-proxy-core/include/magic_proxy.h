#ifndef MAGIC_PROXY_H
#define MAGIC_PROXY_H

#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

// Error codes returned by FFI functions
typedef enum {
    PROXY_SUCCESS = 0,
    PROXY_NULL_POINTER = -1,
    PROXY_INVALID_INPUT = -2,
    PROXY_INITIALIZATION_FAILED = -3,
    PROXY_PARSE_FAILED = -4,
    PROXY_PDF_GENERATION_FAILED = -5,
    PROXY_OUT_OF_MEMORY = -6
} ProxyError;

/**
 * Initialize the proxy generator caches.
 * Must be called before any other FFI functions.
 * 
 * @return PROXY_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t proxy_initialize(void);

/**
 * Generate PDF from decklist text.
 * 
 * @param decklist_cstr Null-terminated C string containing the decklist
 * @param output_buffer Pointer to buffer pointer (will be allocated by this function)
 * @param output_size Pointer to size_t that will receive the buffer size
 * @return PROXY_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - The output buffer is allocated by this function using malloc
 * - Caller must call proxy_free_buffer to free the memory
 * - If function fails, no memory is allocated
 */
int32_t proxy_generate_pdf_from_decklist(
    const char* decklist_cstr,
    uint8_t** output_buffer,
    size_t* output_size
);

/**
 * Free buffer allocated by proxy_generate_pdf_from_decklist.
 * 
 * @param buffer Buffer pointer returned by proxy_generate_pdf_from_decklist
 * 
 * Memory Management:
 * - This function must be called to free buffers from proxy_generate_pdf_from_decklist
 * - Safe to call with NULL pointer (no-op)
 * - Do not call with pointers not returned by proxy_generate_pdf_from_decklist
 */
void proxy_free_buffer(uint8_t* buffer);

/**
 * Get error message for the given error code.
 * 
 * @param error_code Error code returned by other functions
 * @return Static string describing the error (do not free)
 * 
 * Memory Management:
 * - Returns pointer to static string - DO NOT FREE
 * - Returned pointer remains valid for the lifetime of the program
 * - Thread-safe (returns immutable static data)
 */
const char* proxy_get_error_message(int32_t error_code);

/**
 * Simple test function to verify FFI is working.
 * 
 * @return Always returns 42
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t proxy_test_connection(void);

#ifdef __cplusplus
}
#endif

#endif // MAGIC_PROXY_H