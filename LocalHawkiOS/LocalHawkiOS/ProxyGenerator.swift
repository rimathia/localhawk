import Foundation

struct CacheStatistics {
    let count: UInt32
    let sizeMB: Double
}

// MARK: - Print Selection Data Models

enum DoubleFaceMode: Int32, CaseIterable {
    case frontOnly = 0
    case backOnly = 1
    case bothSides = 2
    
    var displayName: String {
        switch self {
        case .frontOnly: return "Front face only"
        case .backOnly: return "Back face only"
        case .bothSides: return "Both sides"
        }
    }
}

struct DecklistEntryData {
    let multiple: Int32
    let name: String
    let set: String?
    let language: String?
    let faceMode: DoubleFaceMode
    let sourceLineNumber: Int32?
    
    init(multiple: Int32, name: String, set: String? = nil, language: String? = nil, 
         faceMode: DoubleFaceMode = .bothSides, sourceLineNumber: Int32? = nil) {
        self.multiple = multiple
        self.name = name
        self.set = set
        self.language = language
        self.faceMode = faceMode
        self.sourceLineNumber = sourceLineNumber
    }
}

struct CardPrintingData {
    let name: String
    let set: String
    let language: String
    let borderCropURL: String
    let backSideURL: String?
    
    init(name: String, set: String, language: String, borderCropURL: String, backSideURL: String? = nil) {
        self.name = name
        self.set = set
        self.language = language
        self.borderCropURL = borderCropURL
        self.backSideURL = backSideURL
    }
}

struct CardSearchResultData {
    let cards: [CardPrintingData]
    
    init(cards: [CardPrintingData]) {
        self.cards = cards
    }
}

enum ProxyGeneratorError: Error, LocalizedError {
    case initializationFailed
    case nullPointer
    case invalidInput
    case parseFailed
    case pdfGenerationFailed
    case outOfMemory
    case imageNotCached
    case imageCacheFailed
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
        case .imageNotCached:
            return "Image is not cached"
        case .imageCacheFailed:
            return "Failed to retrieve image from cache"
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
    
    // MARK: - Print Selection Functions
    
    /// Parse decklist and start background image loading (fire and forget)
    /// This function parses the decklist, starts background loading, and returns immediately
    /// - Parameters:
    ///   - decklist: The decklist text containing card names
    ///   - globalFaceMode: Global face mode setting
    /// - Returns: Result containing array of decklist entries or error
    static func parseAndStartBackgroundLoading(
        _ decklist: String,
        globalFaceMode: DoubleFaceMode = .bothSides
    ) -> Result<[DecklistEntryData], ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let decklistCString = decklist.cString(using: .utf8) else {
            return .failure(.invalidInput)
        }
        
        // Call the simple FFI function
        let result = localhawk_parse_and_start_background_loading(
            decklistCString,
            globalFaceMode.rawValue
        )
        
        // Check for errors
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        
        // For now, we'll still call the separate parse function to get the entries
        // In the future, we could modify the FFI to return the parsed entries
        return parseAndResolveDecklist(decklist, globalFaceMode: globalFaceMode)
    }

    /// Parse decklist and return resolved entries
    /// - Parameters:
    ///   - decklist: The decklist text containing card names
    ///   - globalFaceMode: Global face mode setting
    /// - Returns: Result containing array of decklist entries or error
    static func parseAndResolveDecklist(
        _ decklist: String, 
        globalFaceMode: DoubleFaceMode = .bothSides
    ) -> Result<[DecklistEntryData], ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let decklistCString = decklist.cString(using: .utf8) else {
            return .failure(.invalidInput)
        }
        
        var entriesPtr: UnsafeMutablePointer<DecklistEntry>?
        var count: Int = 0
        
        // Call the FFI function
        let result = localhawk_parse_and_resolve_decklist(
            decklistCString,
            globalFaceMode.rawValue,
            &entriesPtr,
            &count
        )
        
        // Check for errors
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        
        // Ensure we got valid data
        guard let entriesPtr = entriesPtr, count > 0 else {
            return .success([]) // Empty decklist is valid
        }
        
        // Ensure memory is freed regardless of how this scope exits
        defer { localhawk_free_decklist_entries(entriesPtr, count) }
        
        // Convert C structures to Swift objects
        var entries: [DecklistEntryData] = []
        for i in 0..<count {
            let entry = entriesPtr[i]
            
            let name = String(cString: entry.name)
            let set = entry.set != nil ? String(cString: entry.set) : nil
            let language = entry.language != nil ? String(cString: entry.language) : nil
            let faceMode = DoubleFaceMode(rawValue: entry.face_mode) ?? .bothSides
            let sourceLineNumber = entry.source_line_number >= 0 ? entry.source_line_number : nil
            
            entries.append(DecklistEntryData(
                multiple: entry.multiple,
                name: name,
                set: set,
                language: language,
                faceMode: faceMode,
                sourceLineNumber: sourceLineNumber
            ))
        }
        
        return .success(entries)
    }
    
    /// Search for all printings of a specific card
    /// - Parameter cardName: The card name to search for
    /// - Returns: Result containing search results or error
    static func searchCardPrintings(_ cardName: String) -> Result<CardSearchResultData, ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let cardNameCString = cardName.cString(using: .utf8) else {
            return .failure(.invalidInput)
        }
        
        var resultPtr: UnsafeMutablePointer<CardSearchResult>?
        
        // Call the FFI function
        let result = localhawk_search_card_printings(cardNameCString, &resultPtr)
        
        // Check for errors
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        
        // Ensure we got valid data
        guard let resultPtr = resultPtr else {
            return .success(CardSearchResultData(cards: [])) // No results is valid
        }
        
        // Ensure memory is freed regardless of how this scope exits
        defer { localhawk_free_card_search_result(resultPtr) }
        
        let searchResult = resultPtr.pointee
        
        // Convert C structures to Swift objects
        var cards: [CardPrintingData] = []
        for i in 0..<searchResult.count {
            let card = searchResult.cards[i]
            
            let name = String(cString: card.name)
            let set = String(cString: card.set)
            let language = String(cString: card.language)
            let borderCropURL = String(cString: card.border_crop)
            let backSideURL = card.back_side != nil ? String(cString: card.back_side) : nil
            
            cards.append(CardPrintingData(
                name: name,
                set: set,
                language: language,
                borderCropURL: borderCropURL,
                backSideURL: backSideURL
            ))
        }
        
        return .success(CardSearchResultData(cards: cards))
    }
    
    // MARK: - Image Cache Functions
    
    /// Get cached image data for a given URL
    /// - Parameter imageURL: The image URL to retrieve from cache
    /// - Returns: Result containing image data if cached, or error if not cached/failed
    static func getCachedImageData(for imageURL: String) -> Result<Data, ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let imageURLCString = imageURL.cString(using: .utf8) else {
            return .failure(.invalidInput)
        }
        
        var buffer: UnsafeMutablePointer<UInt8>?
        var size: Int = 0
        
        // Call the FFI function
        let result = localhawk_get_cached_image_bytes(imageURLCString, &buffer, &size)
        
        // Check for specific return codes
        switch result {
        case 0: // LOCALHAWK_SUCCESS - image is cached
            guard let buffer = buffer, size > 0 else {
                return .failure(.imageCacheFailed) // Unexpected - success but no data
            }
            
            // Ensure buffer is freed regardless of how this scope exits
            defer { localhawk_free_buffer(buffer) }
            
            // Create Data object from the buffer
            let data = Data(bytes: buffer, count: size)
            return .success(data)
            
        case -4: // LOCALHAWK_PARSE_FAILED - image not cached (expected case)
            return .failure(.imageNotCached)
            
        default: // Other error codes
            return .failure(convertErrorCode(result))
        }
    }
    
    /// Check if an image is cached without retrieving the data
    /// - Parameter imageURL: The image URL to check
    /// - Returns: true if cached, false if not cached or error
    static func isImageCached(for imageURL: String) -> Bool {
        // Ensure initialization
        guard initialize() else {
            return false
        }
        
        // Convert Swift string to C string
        guard let imageURLCString = imageURL.cString(using: .utf8) else {
            return false
        }
        
        // Call the FFI function
        let result = localhawk_is_image_cached(imageURLCString)
        
        // Return true only if result is 1 (TRUE)
        return result == 1
    }
    
    // MARK: - Background Loading Functions
    
    /// Loading phase for background loading progress
    enum BackgroundLoadingPhase: Int32 {
        case selected = 0     // Loading selected printings (based on set/lang hints)
        case alternatives = 1 // Loading alternative printings
        case completed = 2    // All done
        
        var displayName: String {
            switch self {
            case .selected: return "Loading selected printings"
            case .alternatives: return "Loading alternatives"
            case .completed: return "Completed"
            }
        }
    }
    
    /// Background loading progress data
    struct BackgroundLoadingProgress {
        let phase: BackgroundLoadingPhase
        let currentEntry: Int
        let totalEntries: Int
        let selectedLoaded: Int
        let alternativesLoaded: Int
        let totalAlternatives: Int
        let errorCount: Int
        
        var progressText: String {
            switch phase {
            case .selected:
                return "Loading selected printings (\(selectedLoaded)/\(totalEntries))"
            case .alternatives:
                return "Loading alternatives (\(alternativesLoaded)/\(totalAlternatives))"
            case .completed:
                return "Loading complete"
            }
        }
        
        var progressFraction: Double {
            switch phase {
            case .selected:
                return totalEntries > 0 ? Double(selectedLoaded) / Double(totalEntries) * 0.5 : 0.0
            case .alternatives:
                let selectedPhaseProgress = 0.5
                let alternativesProgress = totalAlternatives > 0 ? Double(alternativesLoaded) / Double(totalAlternatives) * 0.5 : 0.0
                return selectedPhaseProgress + alternativesProgress
            case .completed:
                return 1.0
            }
        }
    }
    
    /// Handle for background loading task
    typealias BackgroundLoadingHandle = Int
    
    /// Start background image loading for decklist entries
    /// - Parameter entries: Array of decklist entries to load images for
    /// - Returns: Result containing handle ID for tracking progress, or error
    static func startBackgroundLoading(for entries: [DecklistEntryData]) -> Result<BackgroundLoadingHandle, ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        guard !entries.isEmpty else {
            return .failure(.invalidInput)
        }
        
        // Convert Swift entries to C structures for FFI call
        var cEntries: [DecklistEntry] = []
        var allocatedStrings: [UnsafeMutablePointer<CChar>] = []
        
        // Ensure all allocated strings are freed
        defer {
            for stringPtr in allocatedStrings {
                free(stringPtr)
            }
        }
        
        for entry in entries {
            // Allocate C strings
            let nameCString = strdup(entry.name)
            let setCString = entry.set.flatMap { strdup($0) }
            let languageCString = entry.language.flatMap { strdup($0) }
            
            // Keep track for cleanup
            if let nameCString = nameCString { 
                allocatedStrings.append(nameCString) 
            }
            if let setCString = setCString { 
                allocatedStrings.append(setCString) 
            }
            if let languageCString = languageCString { 
                allocatedStrings.append(languageCString) 
            }
            
            let cEntry = DecklistEntry(
                multiple: entry.multiple,
                name: nameCString,
                set: setCString,
                language: languageCString,
                face_mode: entry.faceMode.rawValue,
                source_line_number: entry.sourceLineNumber ?? -1
            )
            
            cEntries.append(cEntry)
        }
        
        // Call the real FFI function
        var handleId: BackgroundLoadHandleId = 0
        let result = localhawk_start_background_loading(cEntries, cEntries.count, &handleId)
        
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        
        return .success(handleId)
    }
    
    /// Get progress for a background loading task
    /// - Parameter handleId: Handle ID returned from startBackgroundLoading
    /// - Returns: Progress data if available, nil if no new progress
    static func getBackgroundLoadingProgress(for handleId: BackgroundLoadingHandle) -> BackgroundLoadingProgress? {
        var progress = BackgroundLoadProgress(
            phase: LOCALHAWK_LOADING_SELECTED,
            current_entry: 0,
            total_entries: 0,
            selected_loaded: 0,
            alternatives_loaded: 0,
            total_alternatives: 0,
            error_count: 0
        )
        var hasProgress: Int32 = 0
        
        let result = localhawk_get_background_progress(BackgroundLoadHandleId(handleId), &progress, &hasProgress)
        
        guard result == 0 && hasProgress == 1 else {
            return nil
        }
        
        let phase = BackgroundLoadingPhase(rawValue: Int32(progress.phase.rawValue)) ?? .selected
        
        return BackgroundLoadingProgress(
            phase: phase,
            currentEntry: progress.current_entry,
            totalEntries: progress.total_entries,
            selectedLoaded: progress.selected_loaded,
            alternativesLoaded: progress.alternatives_loaded,
            totalAlternatives: progress.total_alternatives,
            errorCount: progress.error_count
        )
    }
    
    /// Cancel background loading task
    /// - Parameter handleId: Handle ID to cancel
    /// - Returns: Success or error
    static func cancelBackgroundLoading(for handleId: BackgroundLoadingHandle) -> Result<Void, ProxyGeneratorError> {
        let result = localhawk_cancel_background_loading(BackgroundLoadHandleId(handleId))
        guard result == 0 else {
            return .failure(convertErrorCode(result))
        }
        return .success(())
    }
    
    /// Check if background loading task is finished
    /// - Parameter handleId: Handle ID to check
    /// - Returns: true if finished, false if still running
    static func isBackgroundLoadingFinished(for handleId: BackgroundLoadingHandle) -> Bool {
        return localhawk_is_background_loading_finished(BackgroundLoadHandleId(handleId)) == 1
    }
}