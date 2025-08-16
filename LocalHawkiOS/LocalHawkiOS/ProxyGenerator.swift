import Foundation

struct CacheStatistics {
    let count: UInt32
    let sizeMB: Double
}

enum ProxyGeneratorError: Error, LocalizedError {
    case initializationFailed
    case nullPointer
    case invalidInput
    case parseFailed
    case pdfGenerationFailed
    case outOfMemory
    case unknownError(Int32)
    
    var errorDescription: String? {
        switch self {
        case .initializationFailed:
            return "Failed to initialize the proxy generator"
        case .nullPointer:
            return "Internal error: null pointer"
        case .invalidInput:
            return "Invalid input string"
        case .parseFailed:
            return "Failed to parse decklist or no valid cards found"
        case .pdfGenerationFailed:
            return "Failed to generate PDF"
        case .outOfMemory:
            return "Out of memory"
        case .unknownError(let code):
            return "Unknown error (code: \(code))"
        }
    }
}

class ProxyGenerator {
    private static var isInitialized = false
    
    /// Initialize the proxy generator caches
    /// Must be called before any other operations
    @discardableResult
    static func initialize() -> Bool {
        guard !isInitialized else { return true }
        
        let result = localhawk_initialize()
        isInitialized = (result == 0)
        
        if !isInitialized {
            print("ProxyGenerator initialization failed with code: \(result)")
        }
        
        return isInitialized
    }
    
    /// Simple test function to verify FFI is working
    static func testConnection() -> Int32 {
        return localhawk_test_connection()
    }
    
    /// Generate PDF from decklist text
    /// - Parameter decklist: The decklist text containing card names
    /// - Returns: Result containing PDF data or error
    static func generatePDF(from decklist: String) -> Result<Data, ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let decklistCString = decklist.cString(using: .utf8) else {
            return .failure(.invalidInput)
        }
        
        var buffer: UnsafeMutablePointer<UInt8>?
        var size: Int = 0
        
        // Call the FFI function
        let result = localhawk_generate_pdf_from_decklist(
            decklistCString,
            &buffer,
            &size
        )
        
        // Check for errors
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        
        // Ensure we got valid data
        guard let buffer = buffer, size > 0 else {
            return .failure(.pdfGenerationFailed)
        }
        
        // Ensure buffer is freed regardless of how this scope exits
        defer { localhawk_free_buffer(buffer) }
        
        // Create Data object from the buffer
        let data = Data(bytes: buffer, count: size)
        
        return .success(data)
    }
    
    /// Convert C error code to Swift error
    private static func convertErrorCode(_ code: Int32) -> ProxyGeneratorError {
        switch code {
        case -1:
            return .nullPointer
        case -2:
            return .invalidInput
        case -3:
            return .initializationFailed
        case -4:
            return .parseFailed
        case -5:
            return .pdfGenerationFailed
        case -6:
            return .outOfMemory
        default:
            return .unknownError(code)
        }
    }
    
    /// Get error message for a given error code
    static func getErrorMessage(for errorCode: Int32) -> String {
        let messagePtr = localhawk_get_error_message(errorCode)
        guard let messagePtr = messagePtr else {
            return "Unknown error"
        }
        // Note: localhawk_get_error_message returns static strings - no need to free
        return String(cString: messagePtr)
    }
    
    // MARK: - Cache Statistics
    
    /// Get image cache statistics
    static func getImageCacheStats() -> CacheStatistics {
        let stats = localhawk_get_image_cache_stats()
        return CacheStatistics(count: stats.count, sizeMB: stats.size_mb)
    }
    
    /// Get search results cache statistics
    static func getSearchCacheStats() -> CacheStatistics {
        let stats = localhawk_get_search_cache_stats()
        return CacheStatistics(count: stats.count, sizeMB: stats.size_mb)
    }
    
    /// Get card names cache statistics
    static func getCardNamesCacheStats() -> CacheStatistics {
        let stats = localhawk_get_card_names_cache_stats()
        return CacheStatistics(count: stats.count, sizeMB: stats.size_mb)
    }
    
    /// Clear image cache
    static func clearImageCache() -> Result<Void, ProxyGeneratorError> {
        let result = localhawk_clear_image_cache()
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        return .success(())
    }
    
    /// Update card names database from Scryfall API
    static func updateCardNames() -> Result<Void, ProxyGeneratorError> {
        let result = localhawk_update_card_names()
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        return .success(())
    }
    
    /// Save all in-memory caches to disk
    static func saveCaches() -> Result<Void, ProxyGeneratorError> {
        let result = localhawk_save_caches()
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        return .success(())
    }
    
    // MARK: - Cache Paths
    
    /// Get the image cache directory path
    static func getImageCachePath() -> String? {
        guard let cString = localhawk_get_image_cache_path() else {
            return nil
        }
        defer { localhawk_free_string(cString) }
        return String(cString: cString)
    }
    
    /// Get the search results cache file path
    static func getSearchCachePath() -> String? {
        guard let cString = localhawk_get_search_cache_path() else {
            return nil
        }
        defer { localhawk_free_string(cString) }
        return String(cString: cString)
    }
    
    /// Get the card names cache file path
    static func getCardNamesCachePath() -> String? {
        guard let cString = localhawk_get_card_names_cache_path() else {
            return nil
        }
        defer { localhawk_free_string(cString) }
        return String(cString: cString)
    }
}