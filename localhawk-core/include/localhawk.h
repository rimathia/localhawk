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

//==============================================================================
// Print Selection & Preview FFI Extensions  
//==============================================================================

/**
 * C-compatible decklist entry structure
 */
typedef struct {
    int32_t multiple;           // Number of copies
    char* name;                 // Card name
    char* set;                  // Set code (NULL if not specified)
    char* language;             // Language code (NULL if not specified)
    int32_t face_mode;          // DoubleFaceMode: 0=FrontOnly, 1=BackOnly, 2=BothSides
    int32_t source_line_number; // Source line number (-1 if not specified)
} DecklistEntry;

/**
 * C-compatible card printing structure
 */
typedef struct {
    char* name;        // Card name
    char* set;         // Set code
    char* language;    // Language code
    char* border_crop; // Front face image URL
    char* back_side;   // Back face image URL (NULL if no back side)
} CardPrinting;

/**
 * C-compatible card search result
 */
typedef struct {
    CardPrinting* cards; // Array of card printings
    size_t count;        // Number of cards in array
} CardSearchResult;

/**
 * Parse decklist and return resolved entries.
 * 
 * @param decklist_cstr Null-terminated C string containing the decklist
 * @param global_face_mode Global face mode: 0=FrontOnly, 1=BackOnly, 2=BothSides
 * @param output_entries Pointer to DecklistEntry array pointer (allocated by function)
 * @param output_count Pointer to size_t that will receive the array size
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - The output array and all strings are allocated by this function
 * - Caller must call localhawk_free_decklist_entries to free the memory
 * - If function fails, no memory is allocated
 */
int32_t localhawk_parse_and_resolve_decklist(
    const char* decklist_cstr,
    int32_t global_face_mode,
    DecklistEntry** output_entries,
    size_t* output_count
);

/**
 * Search for all printings of a specific card.
 * 
 * @param card_name_cstr Null-terminated C string containing the card name
 * @param output_result Pointer to CardSearchResult pointer (allocated by function)
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - The result structure and all strings are allocated by this function
 * - Caller must call localhawk_free_card_search_result to free the memory
 * - If function fails, no memory is allocated
 */
int32_t localhawk_search_card_printings(
    const char* card_name_cstr,
    CardSearchResult** output_result
);

/**
 * Free decklist entries array allocated by localhawk_parse_and_resolve_decklist.
 * 
 * @param entries Array pointer returned by localhawk_parse_and_resolve_decklist
 * @param count Number of entries in the array
 * 
 * Memory Management:
 * - This function must be called to free arrays from localhawk_parse_and_resolve_decklist
 * - Safe to call with NULL pointer (no-op)
 * - Frees the array and all contained strings
 */
void localhawk_free_decklist_entries(DecklistEntry* entries, size_t count);

/**
 * Free card search result allocated by localhawk_search_card_printings.
 * 
 * @param result Result pointer returned by localhawk_search_card_printings
 * 
 * Memory Management:
 * - This function must be called to free results from localhawk_search_card_printings
 * - Safe to call with NULL pointer (no-op)
 * - Frees the result structure and all contained strings
 */
void localhawk_free_card_search_result(CardSearchResult* result);

/**
 * Get cached image bytes for a given URL.
 * 
 * @param image_url_cstr Null-terminated C string containing the image URL
 * @param output_buffer Pointer to buffer pointer (will be allocated by this function)
 * @param output_size Pointer to size_t that will receive the buffer size
 * @return LOCALHAWK_SUCCESS if image is cached, LOCALHAWK_PARSE_FAILED if not cached, negative error code on failure
 * 
 * Memory Management:
 * - The output buffer is allocated by this function using malloc
 * - Caller must call localhawk_free_buffer to free the memory
 * - If function fails or image not cached, no memory is allocated
 */
int32_t localhawk_get_cached_image_bytes(
    const char* image_url_cstr,
    uint8_t** output_buffer,
    size_t* output_size
);

/**
 * Check if an image is cached without retrieving the bytes.
 * 
 * @param image_url_cstr Null-terminated C string containing the image URL
 * @return 1 if image is cached, 0 if not cached, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 */
int32_t localhawk_is_image_cached(const char* image_url_cstr);

//==============================================================================
// Background Loading FFI Extensions
//==============================================================================

/**
 * Parse decklist and start background image loading (fire and forget).
 * This function parses the decklist, starts background loading, and returns immediately.
 * Background image loading happens asynchronously without tracking.
 * 
 * @param decklist_cstr Null-terminated C string containing the decklist
 * @param global_face_mode Global face mode: 0=FrontOnly, 1=BackOnly, 2=BothSides
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - No cleanup required
 * - Background loading happens asynchronously after function returns
 */
int32_t localhawk_parse_and_start_background_loading(
    const char* decklist_cstr,
    int32_t global_face_mode
);

/**
 * Loading phase enum for background loading progress
 */
typedef enum {
    LOCALHAWK_LOADING_SELECTED = 0,     // Loading selected printings (based on set/lang hints)
    LOCALHAWK_LOADING_ALTERNATIVES = 1, // Loading alternative printings
    LOCALHAWK_LOADING_COMPLETED = 2     // All done
} LocalHawkLoadingPhase;

/**
 * Background loading progress structure
 */
typedef struct {
    LocalHawkLoadingPhase phase;         // Current loading phase
    size_t current_entry;                // Current entry being processed
    size_t total_entries;                // Total entries to process
    size_t selected_loaded;              // Number of selected printings loaded
    size_t alternatives_loaded;          // Number of alternative printings loaded
    size_t total_alternatives;           // Total alternative printings to load
    size_t error_count;                  // Number of errors encountered
} BackgroundLoadProgress;

/**
 * Handle ID for background loading task
 */
typedef size_t BackgroundLoadHandleId;

/**
 * Start background image loading for decklist entries.
 * This will automatically search for card printings and download images in the background.
 * 
 * @param entries Array of decklist entries
 * @param count Number of entries in the array
 * @param handle_id Pointer to handle ID that will be set (for progress tracking)
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - Use localhawk_get_background_progress to track progress
 * - Use localhawk_cancel_background_loading to cancel if needed
 */
int32_t localhawk_start_background_loading(
    const DecklistEntry* entries,
    size_t count,
    BackgroundLoadHandleId* handle_id
);

/**
 * Get progress for a background loading task.
 * 
 * @param handle_id Handle ID returned by localhawk_start_background_loading
 * @param progress Pointer to progress structure (will be filled if progress available)
 * @param has_progress Pointer to int that will be set to 1 if progress available, 0 otherwise
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - Progress structure is filled by value
 * - Call periodically to get latest progress updates
 */
int32_t localhawk_get_background_progress(
    BackgroundLoadHandleId handle_id,
    BackgroundLoadProgress* progress,
    int32_t* has_progress
);

/**
 * Cancel background loading task.
 * 
 * @param handle_id Handle ID of the task to cancel
 * @return LOCALHAWK_SUCCESS on success, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - Task will be cleaned up automatically after cancellation
 */
int32_t localhawk_cancel_background_loading(BackgroundLoadHandleId handle_id);

/**
 * Check if background loading task is finished.
 * 
 * @param handle_id Handle ID of the task to check
 * @return 1 if finished, 0 if still running, negative error code on failure
 * 
 * Memory Management:
 * - No memory is allocated by this function
 * - Finished tasks are automatically cleaned up
 */
int32_t localhawk_is_background_loading_finished(BackgroundLoadHandleId handle_id);

#ifdef __cplusplus
}
#endif

#endif // LOCALHAWK_H