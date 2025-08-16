#ifndef LOCALHAWK_H
#define LOCALHAWK_H

#include <stdint.h>
#include <stdlib.h>

#ifdef __cplusplus
extern "C" {
#endif

// Error codes returned by FFI functions
typedef enum {
    LOCALHAWK_SUCCESS = 0,
    LOCALHAWK_NULL_POINTER = -1,
    LOCALHAWK_INVALID_INPUT = -2,
    LOCALHAWK_INITIALIZATION_FAILED = -3,
    LOCALHAWK_PARSE_FAILED = -4,
    LOCALHAWK_PDF_GENERATION_FAILED = -5,
    LOCALHAWK_OUT_OF_MEMORY = -6
} LocalHawkError;

/**
 * Initialize the proxy generator caches.
 * Must be called before any other FFI functions.
 * 
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_initialize(void);

/**
 * Generate PDF from decklist text.
 * 
 * @param decklist_cstr Null-terminated C string containing the decklist
 * @param output_buffer Pointer to buffer pointer (will be allocated by this function)
 * @param output_size Pointer to size_t that will receive the buffer size
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - The output buffer is allocated by this function using malloc
 * - Caller must call localhawk_free_buffer to free the memory
 * - If function fails, no memory is allocated
 */
int32_t localhawk_generate_pdf_from_decklist(
    const char* decklist_cstr,
    uint8_t** output_buffer,
    size_t* output_size
);

/**
 * Free buffer allocated by localhawk_generate_pdf_from_decklist.
 * 
 * @param buffer Buffer pointer returned by localhawk_generate_pdf_from_decklist
 * 
 * Memory Management:
 * - This function must be called to free buffers from localhawk_generate_pdf_from_decklist
 * - Safe to call with NULL pointer (no-op)
 * - Do not call with pointers not returned by localhawk_generate_pdf_from_decklist
 */
void localhawk_free_buffer(uint8_t* buffer);

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
const char* localhawk_get_error_message(int32_t error_code);

/**
 * Simple test function to verify FFI is working.
 * 
 * @return Always returns 42
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_test_connection(void);

/**
 * Cache statistics structure.
 */
typedef struct {
    uint32_t count;      // Number of items in cache
    double size_mb;      // Size in megabytes
} CacheStats;

/**
 * Get image cache statistics.
 * 
 * @return CacheStats structure with current image cache info
 * 
 * Memory Management:
 * - Returns struct by value (no memory allocation)
 * - No cleanup required
 */
CacheStats localhawk_get_image_cache_stats(void);

/**
 * Get search results cache statistics.
 * 
 * @return CacheStats structure with current search cache info
 * 
 * Memory Management:
 * - Returns struct by value (no memory allocation)
 * - No cleanup required
 */
CacheStats localhawk_get_search_cache_stats(void);

/**
 * Get card names cache statistics.
 * 
 * @return CacheStats structure with current card names cache info
 *         Returns count=0 if cache is not initialized
 * 
 * Memory Management:
 * - Returns struct by value (no memory allocation)
 * - No cleanup required
 */
CacheStats localhawk_get_card_names_cache_stats(void);

/**
 * Clear the image cache.
 * 
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_clear_image_cache(void);

/**
 * Update card names database from Scryfall API.
 * This is a blocking operation that may take several seconds.
 * 
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_update_card_names(void);

/**
 * Save all in-memory caches to disk.
 * This saves image cache and search results cache without shutting down.
 * 
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_save_caches(void);

/**
 * Get the image cache directory path.
 * 
 * @return Pointer to null-terminated string containing the path
 *         Returns NULL on error
 * 
 * Memory Management:
 * - String is allocated by this function
 * - Caller must call localhawk_free_string to free the memory
 */
char* localhawk_get_image_cache_path(void);

/**
 * Get the search results cache file path.
 * 
 * @return Pointer to null-terminated string containing the path
 *         Returns NULL on error
 * 
 * Memory Management:
 * - String is allocated by this function
 * - Caller must call localhawk_free_string to free the memory
 */
char* localhawk_get_search_cache_path(void);

/**
 * Get the card names cache file path.
 * 
 * @return Pointer to null-terminated string containing the path
 *         Returns NULL on error
 * 
 * Memory Management:
 * - String is allocated by this function
 * - Caller must call localhawk_free_string to free the memory
 */
char* localhawk_get_card_names_cache_path(void);

/**
 * Free a string allocated by localhawk_get_*_path functions.
 * 
 * @param ptr String pointer returned by localhawk_get_*_path functions
 * 
 * Memory Management:
 * - This function must be called to free strings from localhawk_get_*_path
 * - Safe to call with NULL pointer (no-op)
 * - Do not call with pointers not returned by localhawk_get_*_path functions
 */
void localhawk_free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif // LOCALHAWK_H