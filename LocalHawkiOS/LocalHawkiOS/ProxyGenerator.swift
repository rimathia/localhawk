import Foundation

// MARK: - Debug Logging Helper

/// Debug print that only outputs in debug builds
private func debugPrint(_ items: Any..., separator: String = " ", terminator: String = "\n") {
    #if DEBUG
    print(items.map { "\($0)" }.joined(separator: separator), terminator: terminator)
    #endif
}

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
    var set: String?      // Make mutable for print selection updates
    var language: String? // Make mutable for print selection updates
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

/// Back side type for enhanced FFI
enum BackSideType: UInt32 {
    case none = 0    // No back side
    case dfc = 1     // Double-faced card back side
    case meld = 2    // Meld result card
}

struct CardPrintingData: Equatable {
    let name: String
    let set: String
    let language: String
    let borderCropURL: String
    let backSideURL: String?
    let backSideType: BackSideType    // NEW: Type of back side
    let backSideName: String?         // NEW: Back face name or meld result name

    init(name: String, set: String, language: String, borderCropURL: String, backSideURL: String? = nil, backSideType: BackSideType = .none, backSideName: String? = nil) {
        self.name = name
        self.set = set
        self.language = language
        self.borderCropURL = borderCropURL
        self.backSideURL = backSideURL
        self.backSideType = backSideType
        self.backSideName = backSideName
    }
}

struct CardSearchResultData {
    let cards: [CardPrintingData]
    
    init(cards: [CardPrintingData]) {
        self.cards = cards
    }
}

/// Mutable resolved card that matches desktop pattern exactly
/// This represents a (Card, quantity, face_mode) tuple that can be modified by print selection
class ResolvedCard: ObservableObject {
    @Published var card: CardPrintingData  // The selected card (can be changed by print selection)
    let quantity: UInt32                   // Number of copies
    let faceMode: DoubleFaceMode          // Face mode for this entry

    init(card: CardPrintingData, quantity: UInt32, faceMode: DoubleFaceMode) {
        self.card = card
        self.quantity = quantity
        self.faceMode = faceMode
    }
    
    /// Get image URLs based on face mode (matches core library logic)
    func getImageUrls() -> [String] {
        switch faceMode {
        case .frontOnly:
            return [card.borderCropURL]
        case .backOnly:
            return card.backSideURL.map { [$0] } ?? [card.borderCropURL]
        case .bothSides:
            if let backURL = card.backSideURL {
                return [card.borderCropURL, backURL]
            } else {
                return [card.borderCropURL]
            }
        }
    }
}

/// Wrapper to make the resolved cards array observable
class ResolvedCardsWrapper: ObservableObject {
    @Published var cards: [ResolvedCard]
    
    init(cards: [ResolvedCard]) {
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

// MARK: - Image Cache Notifications

/// Image cache change notification from Rust
struct ImageCacheChangeNotification {
    let changeType: UInt8 // 1=ImageCached, 2=ImageRemoved
    let imageUrl: String
    let timestamp: UInt64
}

class ProxyGenerator {
    private static var isInitialized = false
    
    /// Initialize the proxy generator caches
    /// Must be called before any other operations
    @discardableResult
    static func initialize() -> Bool {
        guard !isInitialized else {
            debugPrint("🟢 [ProxyGenerator] Already initialized")
            return true
        }

        debugPrint("🚀 [ProxyGenerator] Starting initialization...")
        let result = localhawk_initialize()
        isInitialized = (result == 0)

        if isInitialized {
            debugPrint("✅ [ProxyGenerator] Initialization successful")
        } else {
            // Keep error messages for user debugging
            print("❌ [ProxyGenerator] Initialization failed with code: \(result)")
        }
        
        return isInitialized
    }
    
    /// Simple test function to verify FFI is working
    static func testConnection() -> Int32 {
        return localhawk_test_connection()
    }
    
    // MARK: - Image Cache Dispatch Source Notifications
    
    private static var globalImageCacheDispatchSource: DispatchSourceUserDataAdd?
    private static var imageCacheListeners: [UUID: (ImageCacheChangeNotification) -> Void] = [:]
    
    /// C callback function for image cache dispatch source notifications
    private static let imageCacheNotificationCallback: @convention(c) (UnsafeRawPointer?, UnsafePointer<CChar>?) -> Void = { sourcePtr, keyCStr in
        guard let sourcePtr = sourcePtr, let keyCStr = keyCStr else {
            debugPrint("⚠️ [ProxyGenerator] Image cache dispatch callback received null pointer")
            return
        }
        
        let key = String(cString: keyCStr)
        debugPrint("📲 [ProxyGenerator] Image cache dispatch source notification for key: '\(key)'")
        
        // Convert the source pointer back to the dispatch source and trigger it
        let source = Unmanaged<DispatchSourceUserDataAdd>.fromOpaque(sourcePtr).takeUnretainedValue()
        
        // Trigger the dispatch source by merging data
        source.add(data: 1)
        
        debugPrint("🔔 [ProxyGenerator] Triggered image cache dispatch source for key: '\(key)'")
    }
    
    /// Get queued image cache change notifications from Rust
    private static func getQueuedImageCacheChanges() -> [ImageCacheChangeNotification] {
        guard let cArrayPtr = localhawk_get_queued_image_cache_changes() else {
            return []
        }
        
        let cArray = cArrayPtr.pointee
        var changes: [ImageCacheChangeNotification] = []
        
        for i in 0..<Int(cArray.count) {
            let cChange = cArray.changes.advanced(by: i).pointee
            
            let imageUrl = String(cString: cChange.image_url)
            
            let change = ImageCacheChangeNotification(
                changeType: cChange.change_type,
                imageUrl: imageUrl,
                timestamp: cChange.timestamp
            )
            changes.append(change)
        }
        
        // Free the allocated memory
        localhawk_free_image_cache_change_array(cArrayPtr)
        
        return changes
    }
    
    /// Register for image cache change notifications
    /// Returns a listener UUID that can be used to unregister specifically
    @discardableResult
    static func startWatchingImageCache(callback: @escaping (ImageCacheChangeNotification) -> Void) -> UUID {
        guard initialize() else { 
            print("❌ [ProxyGenerator] Failed to initialize for image cache watching")
            return UUID() // Return dummy UUID on failure
        }
        
        let listenerID = UUID()
        debugPrint("📡 [ProxyGenerator] Starting to watch image cache with ID \(listenerID)")
        
        // Create global dispatch source only on first registration
        if globalImageCacheDispatchSource == nil {
            let source = DispatchSource.makeUserDataAddSource(queue: DispatchQueue.main)
            
            source.setEventHandler {
                debugPrint("🔔 [ProxyGenerator] Global image cache dispatch source fired")
                
                // Get all queued image cache change notifications
                let changes = getQueuedImageCacheChanges()
                print("📥 [ProxyGenerator] Processing \(changes.count) image cache change notifications")
                
                // Notify all listeners
                for change in changes {
                    print("🖼️ [ProxyGenerator] Image cache change: \(change.changeType == 1 ? "CACHED" : "REMOVED") - \(change.imageUrl)")
                    for (_, callback) in imageCacheListeners {
                        callback(change)
                    }
                }
            }
            
            source.resume()
            globalImageCacheDispatchSource = source
            
            // Register with Rust
            let sourcePtr = Unmanaged.passUnretained(source).toOpaque()
            let result = localhawk_register_image_cache_dispatch_source(sourcePtr, imageCacheNotificationCallback)
            
            if result != 0 {
                print("❌ [ProxyGenerator] Failed to register image cache dispatch source: \(result)")
                source.cancel()
                globalImageCacheDispatchSource = nil
                return UUID() // Return dummy UUID on failure
            }
            
            print("✅ [ProxyGenerator] Registered image cache dispatch source with Rust")
        }
        
        // Add callback to listeners with UUID key
        imageCacheListeners[listenerID] = callback
        print("📝 [ProxyGenerator] Added image cache listener \(listenerID). Total listeners: \(imageCacheListeners.count)")
        return listenerID
    }
    
    /// Stop watching image cache changes for a specific listener
    static func stopWatchingImageCache(listenerID: UUID) {
        print("🛑 [ProxyGenerator] Stopping image cache watching for ID \(listenerID)")
        
        // Remove specific listener
        imageCacheListeners.removeValue(forKey: listenerID)
        print("📝 [ProxyGenerator] Removed image cache listener \(listenerID). Remaining listeners: \(imageCacheListeners.count)")
        
        // Only unregister and cancel dispatch source when no more listeners
        if imageCacheListeners.isEmpty {
            if let source = globalImageCacheDispatchSource {
                source.cancel()
                globalImageCacheDispatchSource = nil
                
                // Unregister from Rust
                let result = localhawk_unregister_image_cache_dispatch_source()
                if result != 0 {
                    print("⚠️ [ProxyGenerator] Failed to unregister image cache dispatch source: \(result)")
                } else {
                    print("✅ [ProxyGenerator] Unregistered image cache dispatch source from Rust")
                }
            }
        } else {
            print("📝 [ProxyGenerator] Keeping image cache dispatch source active (still have \(imageCacheListeners.count) listeners)")
        }
    }
    
    /// Generate PDF from decklist text
    /// - Parameter decklist: The decklist text containing card names
    /// - Returns: Result containing PDF data or error
    static func generatePDF(from decklist: String) -> Result<Data, ProxyGeneratorError> {
        print("🔄 [ProxyGenerator] Starting PDF generation from decklist...")
        
        // Ensure initialization
        guard initialize() else {
            print("❌ [ProxyGenerator] PDF generation failed - initialization failed")
            return .failure(.initializationFailed)
        }
        
        debugPrint("📝 [ProxyGenerator] Processing decklist with \(decklist.split(separator: "\n").count) lines")
        
        // Convert Swift string to C string
        guard let decklistCString = decklist.cString(using: .utf8) else {
            print("❌ [ProxyGenerator] PDF generation failed - invalid input encoding")
            return .failure(.invalidInput)
        }
        
        var buffer: UnsafeMutablePointer<UInt8>?
        var size: Int = 0
        
        print("🚀 [ProxyGenerator] Calling Rust core for PDF generation...")
        
        // Call the FFI function
        let result = localhawk_generate_pdf_from_decklist(
            decklistCString,
            &buffer,
            &size
        )
        
        // Check for errors
        guard result == 0 else {
            print("❌ [ProxyGenerator] PDF generation failed with code: \(result)")
            return .failure(convertErrorCode(result))
        }
        
        // Ensure we got valid data
        guard let buffer = buffer, size > 0 else {
            print("❌ [ProxyGenerator] PDF generation failed - invalid buffer or size")
            return .failure(.pdfGenerationFailed)
        }
        
        print("📄 [ProxyGenerator] PDF generated successfully - size: \(size) bytes")
        
        // Ensure buffer is freed regardless of how this scope exits
        defer { localhawk_free_buffer(buffer) }
        
        // Create Data object from the buffer
        let data = Data(bytes: buffer, count: size)
        
        print("✅ [ProxyGenerator] PDF data created and ready for use")
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
        print("🗑️ [ProxyGenerator] Clearing image cache...")
        let result = localhawk_clear_image_cache()
        guard result == 0 else {
            print("❌ [ProxyGenerator] Failed to clear image cache with code: \(result)")
            return .failure(convertErrorCode(result))
        }
        print("✅ [ProxyGenerator] Image cache cleared successfully")
        return .success(())
    }
    
    /// Update card names database from Scryfall API
    static func updateCardNames() -> Result<Void, ProxyGeneratorError> {
        print("🔄 [ProxyGenerator] Updating card names database from Scryfall API...")
        let result = localhawk_update_card_names()
        guard result == 0 else {
            print("❌ [ProxyGenerator] Failed to update card names with code: \(result)")
            return .failure(convertErrorCode(result))
        }
        print("✅ [ProxyGenerator] Card names database updated successfully")
        return .success(())
    }
    
    /// Save all in-memory caches to disk
    static func saveCaches() -> Result<Void, ProxyGeneratorError> {
        print("💾 [ProxyGenerator] Saving caches to disk...")
        let result = localhawk_save_caches()
        guard result == 0 else {
            print("❌ [ProxyGenerator] Failed to save caches with code: \(result)")
            return .failure(convertErrorCode(result))
        }
        print("✅ [ProxyGenerator] Caches saved to disk successfully")
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
    
    /// Parse decklist and start background loading
    /// This function follows the desktop pattern: parse → resolve → auto-start background loading
    /// - Parameters:
    ///   - decklist: The decklist text containing card names
    ///   - globalFaceMode: Global face mode setting
    /// - Returns: Result containing tuple of (entries, resolved cards) or error  
    static func parseAndStartBackgroundLoading(
        _ decklist: String,
        globalFaceMode: DoubleFaceMode = .bothSides
    ) -> Result<([DecklistEntryData], [ResolvedCard]), ProxyGeneratorError> {
        debugPrint("🔍 [ProxyGenerator] Parsing decklist, resolving cards, and starting background loading...")
        print("📝 [ProxyGenerator] Decklist has \(decklist.split(separator: "\n").count) lines, face mode: \(globalFaceMode.displayName)")
        
        // Ensure initialization
        guard initialize() else {
            print("❌ [ProxyGenerator] Parse failed - initialization failed")
            return .failure(.initializationFailed)
        }
        
        // Convert Swift string to C string
        guard let decklistCString = decklist.cString(using: .utf8) else {
            print("❌ [ProxyGenerator] Parse failed - invalid input encoding")
            return .failure(.invalidInput)
        }
        
        debugPrint("🚀 [ProxyGenerator] Calling Rust core for parsing and background loading...")
        
        // Prepare output pointers
        var entriesPtr: UnsafeMutablePointer<DecklistEntry>?
        var entriesCount: size_t = 0
        
        // Call the FFI function that returns decklist entries and starts background loading
        let result = localhawk_parse_and_start_background_loading(
            decklistCString,
            globalFaceMode.rawValue,
            &entriesPtr,
            &entriesCount
        )
        
        // Check for errors
        guard result == 0 else {
            print("❌ [ProxyGenerator] Parse and background loading failed with code: \(result)")
            return .failure(convertErrorCode(result))
        }
        
        // Ensure we got entries
        guard let entriesArray = entriesPtr, entriesCount > 0 else {
            debugPrint("✅ [ProxyGenerator] No entries parsed from decklist")
            return .success(([], []))
        }
        
        print("✅ [ProxyGenerator] Got \(entriesCount) entries from Rust core")
        
        // Convert C array to Swift array
        var entries: [DecklistEntryData] = []
        
        for i in 0..<entriesCount {
            let cEntry = entriesArray.advanced(by: i).pointee
            
            // Convert C strings to Swift strings
            let name = String(cString: cEntry.name)
            let set = cEntry.set != nil ? String(cString: cEntry.set) : nil
            let language = cEntry.language != nil ? String(cString: cEntry.language) : nil
            
            // Convert face mode
            let faceMode: DoubleFaceMode
            switch cEntry.face_mode {
            case 0:
                faceMode = .frontOnly
            case 1:
                faceMode = .backOnly
            case 2:
                faceMode = .bothSides
            default:
                faceMode = .bothSides
            }
            
            let entry = DecklistEntryData(
                multiple: Int32(cEntry.multiple),
                name: name,
                set: set,
                language: language,
                faceMode: faceMode,
                sourceLineNumber: Int32(cEntry.source_line_number)
            )
            
            entries.append(entry)
            
            let setStr = set ?? "any"
            let langStr = language ?? "any"
            print("📝 [ProxyGenerator] Entry: '\(name)' (\(setStr)) [\(langStr)] x\(cEntry.multiple) - \(faceMode.displayName)")
        }
        
        // Now get the resolved cards using the new FFI function
        var resolvedCardsPtr: UnsafeMutablePointer<LocalHawkResolvedCard>?
        var resolvedCardsCount: size_t = 0
        
        let resolveResult = localhawk_get_resolved_cards_for_entries(
            entriesArray,
            entriesCount,
            &resolvedCardsPtr,
            &resolvedCardsCount
        )
        
        // Free the entries memory first
        localhawk_free_decklist_entries(entriesPtr, entriesCount)
        
        // Check resolve result
        guard resolveResult == 0 else {
            print("❌ [ProxyGenerator] Failed to get resolved cards with code: \(resolveResult)")
            return .failure(convertErrorCode(resolveResult))
        }
        
        // Convert resolved cards to Swift
        var resolvedCards: [ResolvedCard] = []
        if let resolvedCardsArray = resolvedCardsPtr, resolvedCardsCount > 0 {
            for i in 0..<resolvedCardsCount {
                let cCard = resolvedCardsArray.advanced(by: i).pointee
                
                let name = String(cString: cCard.name)
                let setCode = String(cString: cCard.set_code)
                let language = String(cString: cCard.language)
                let borderCropURL = String(cString: cCard.border_crop_url)
                let backBorderCropURL = cCard.back_border_crop_url != nil ? String(cString: cCard.back_border_crop_url) : nil
                
                let faceMode: DoubleFaceMode
                switch cCard.face_mode {
                case LOCALHAWK_FACE_MODE_FRONT_ONLY:
                    faceMode = .frontOnly
                case LOCALHAWK_FACE_MODE_BACK_ONLY:
                    faceMode = .backOnly
                case LOCALHAWK_FACE_MODE_BOTH_SIDES:
                    faceMode = .bothSides
                default:
                    faceMode = .bothSides
                }
                
                let cardPrinting = CardPrintingData(
                    name: name,
                    set: setCode,
                    language: language,
                    borderCropURL: borderCropURL,
                    backSideURL: backBorderCropURL,
                    backSideType: .none,  // This is from CResolvedCard, not enhanced CardPrinting
                    backSideName: nil     // This is from CResolvedCard, not enhanced CardPrinting
                )
                
                let resolvedCard = ResolvedCard(
                    card: cardPrinting,
                    quantity: cCard.quantity,
                    faceMode: faceMode
                )
                
                resolvedCards.append(resolvedCard)
                
                print("🎯 [ProxyGenerator] Resolved card: '\(name)' (\(setCode)) [\(language)] x\(cCard.quantity)")
            }
            
            // Free resolved cards memory
            localhawk_free_resolved_cards(resolvedCardsPtr, resolvedCardsCount)
        }
        
        debugPrint("✅ [ProxyGenerator] Parsed \(entries.count) entries, resolved \(resolvedCards.count) cards, background loading started automatically")
        return .success((entries, resolvedCards))
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
            let backSideType: BackSideType = {
                switch card.back_type {
                case BACK_SIDE_NONE:
                    return .none
                case BACK_SIDE_DFC:
                    return .dfc
                case BACK_SIDE_MELD:
                    return .meld
                default:
                    return .none
                }
            }()
            let backSideName = card.back_name != nil ? String(cString: card.back_name) : nil

            cards.append(CardPrintingData(
                name: name,
                set: set,
                language: language,
                borderCropURL: borderCropURL,
                backSideURL: backSideURL,
                backSideType: backSideType,
                backSideName: backSideName
            ))
        }
        
        return .success(CardSearchResultData(cards: cards))
    }
    
    /// Generate PDF from an array of DecklistEntryData structures
    /// This allows PDF generation with modified entries (e.g., after print selection)
    /// - Parameter entries: Array of decklist entries with potentially modified set/language selections
    /// - Returns: Result containing PDF data or error
    static func generatePDFFromEntries(_ entries: [DecklistEntryData]) -> Result<Data, ProxyGeneratorError> {
        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }
        
        guard !entries.isEmpty else {
            return .failure(.invalidInput)
        }
        
        // Convert Swift DecklistEntryData to C DecklistEntry structures
        var cEntries: [DecklistEntry] = []
        var cStrings: [UnsafeMutablePointer<CChar>] = [] // Keep track of allocated strings
        
        // Helper function to create C string and track it for cleanup
        func createCString(_ string: String?) -> UnsafeMutablePointer<CChar>? {
            guard let string = string else { return nil }
            let cString = strdup(string)
            if let cString = cString {
                cStrings.append(cString)
            }
            return cString
        }
        
        for entry in entries {
            let cEntry = DecklistEntry(
                multiple: entry.multiple,
                name: createCString(entry.name)!,
                set: createCString(entry.set),
                language: createCString(entry.language),
                face_mode: entry.faceMode.rawValue,
                source_line_number: entry.sourceLineNumber ?? -1
            )
            cEntries.append(cEntry)
        }
        
        // Ensure all C strings are freed regardless of how this scope exits
        defer {
            for cString in cStrings {
                free(cString)
            }
        }
        
        var buffer: UnsafeMutablePointer<UInt8>?
        var size: Int = 0
        
        // Call the FFI function
        let result = localhawk_generate_pdf_from_entries(
            cEntries,
            cEntries.count,
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

    // MARK: - Card Expansion Functions

    /// Expand a single resolved card to its image URLs using Rust logic.
    /// This ensures 100% consistency with PDF generation.
    /// - Parameter resolvedCard: The resolved card to expand
    /// - Returns: Array of image URLs in the exact same order as PDF generation
    static func expandSingleCard(_ resolvedCard: ResolvedCard) -> [String] {
        // Log the input for debugging
        print("🍎 [Swift] expandSingleCard: '\(resolvedCard.card.name)' qty=\(resolvedCard.quantity) face_mode=\(resolvedCard.faceMode)")

        // Ensure initialization
        guard initialize() else {
            return []
        }

        // Prepare C strings
        guard let nameCString = resolvedCard.card.name.cString(using: .utf8),
              let setCString = resolvedCard.card.set.cString(using: .utf8),
              let languageCString = resolvedCard.card.language.cString(using: .utf8),
              let borderCropCString = resolvedCard.card.borderCropURL.cString(using: .utf8) else {
            return []
        }

        // Prepare back side URL (nullable)
        let backSideResult: Int32
        if let backSideURL = resolvedCard.card.backSideURL,
           let backSideCString = backSideURL.cString(using: .utf8) {
            var urlsPtr: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
            var count: size_t = 0

            backSideResult = localhawk_expand_single_card(
                nameCString,
                setCString,
                languageCString,
                borderCropCString,
                backSideCString,
                resolvedCard.quantity,
                resolvedCard.faceMode.rawValue,
                &urlsPtr,
                &count
            )

            guard backSideResult == 0, let urlsArray = urlsPtr, count > 0 else {
                return []
            }

            // Convert C strings to Swift strings
            var imageUrls: [String] = []
            for i in 0..<count {
                if let cString = urlsArray[i] {
                    let url = String(cString: cString)
                    imageUrls.append(url)
                }
            }

            // Free the allocated memory
            localhawk_free_image_urls(urlsArray, count)

            print("🍎 [Swift] expandSingleCard result (with back): \(imageUrls.count) URLs")
            for (i, url) in imageUrls.enumerated() {
                print("  [\(i)] \(url)")
            }

            return imageUrls
        } else {
            // No back side URL - pass nil
            var urlsPtr: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
            var count: size_t = 0

            backSideResult = localhawk_expand_single_card(
                nameCString,
                setCString,
                languageCString,
                borderCropCString,
                nil,
                resolvedCard.quantity,
                resolvedCard.faceMode.rawValue,
                &urlsPtr,
                &count
            )

            guard backSideResult == 0, let urlsArray = urlsPtr, count > 0 else {
                return []
            }

            // Convert C strings to Swift strings
            var imageUrls: [String] = []
            for i in 0..<count {
                if let cString = urlsArray[i] {
                    let url = String(cString: cString)
                    imageUrls.append(url)
                }
            }

            // Free the allocated memory
            localhawk_free_image_urls(urlsArray, count)

            print("🍎 [Swift] expandSingleCard result (no back): \(imageUrls.count) URLs")
            for (i, url) in imageUrls.enumerated() {
                print("  [\(i)] \(url)")
            }

            return imageUrls
        }
    }

    /// Resolve decklist entries to actual cards with image URLs (step 2 of desktop pattern)
    /// - Parameter entries: Array of DecklistEntryData from parsing
    /// - Returns: Result containing resolved cards or error
    static func resolveEntriesToCards(_ entries: [DecklistEntryData]) -> Result<[ResolvedCard], ProxyGeneratorError> {
        print("🔧 [ProxyGenerator] Resolving \(entries.count) entries to cards...")

        // Ensure initialization
        guard initialize() else {
            return .failure(.initializationFailed)
        }

        guard !entries.isEmpty else {
            return .failure(.invalidInput)
        }

        // Convert Swift DecklistEntryData to C DecklistEntry structures
        var cEntries: [DecklistEntry] = []
        var cStrings: [UnsafeMutablePointer<CChar>] = [] // Keep track of allocated strings

        // Helper function to create C string and track it for cleanup
        func createCString(_ string: String?) -> UnsafeMutablePointer<CChar>? {
            guard let string = string else { return nil }
            let cString = strdup(string)
            if let cString = cString {
                cStrings.append(cString)
            }
            return cString
        }

        defer {
            // Clean up allocated C strings
            for cString in cStrings {
                free(cString)
            }
        }

        // Convert entries to C structures
        for entry in entries {
            let cEntry = DecklistEntry(
                multiple: entry.multiple,
                name: createCString(entry.name),
                set: createCString(entry.set),
                language: createCString(entry.language),
                face_mode: Int32(entry.faceMode.rawValue),
                source_line_number: entry.sourceLineNumber.map { Int32($0) } ?? -1
            )
            cEntries.append(cEntry)
        }

        // Call FFI to resolve entries to cards (using existing function)
        var cardsPtr: UnsafeMutablePointer<LocalHawkResolvedCard>?
        var cardsCount: size_t = 0

        let result = localhawk_get_resolved_cards_for_entries(
            cEntries, cEntries.count,
            &cardsPtr, &cardsCount
        )

        guard result == 0, let cards = cardsPtr, cardsCount > 0 else {
            print("❌ [ProxyGenerator] Failed to resolve entries to cards")
            return .failure(.unknownError(result))
        }

        // Convert C resolved cards to Swift ResolvedCard objects
        var resolvedCards: [ResolvedCard] = []
        for i in 0..<cardsCount {
            let cCard = cards[i]

            let backSideType: BackSideType = {
                switch cCard.back_type {
                case BACK_SIDE_NONE:
                    return .none
                case BACK_SIDE_DFC:
                    return .dfc
                case BACK_SIDE_MELD:
                    return .meld
                default:
                    return .none
                }
            }()
            let backSideName = cCard.back_name != nil ? String(cString: cCard.back_name) : nil

            let cardPrinting = CardPrintingData(
                name: String(cString: cCard.name),
                set: String(cString: cCard.set_code),
                language: String(cString: cCard.language),
                borderCropURL: String(cString: cCard.border_crop_url),
                backSideURL: cCard.back_border_crop_url != nil ? String(cString: cCard.back_border_crop_url!) : nil,
                backSideType: backSideType,
                backSideName: backSideName
            )

            let resolvedCard = ResolvedCard(
                card: cardPrinting,
                quantity: cCard.quantity,
                faceMode: DoubleFaceMode(rawValue: Int32(cCard.face_mode.rawValue)) ?? .bothSides
            )

            resolvedCards.append(resolvedCard)
        }

        // Free the C array
        localhawk_free_resolved_cards(cardsPtr, cardsCount)

        print("✅ [ProxyGenerator] Successfully resolved \(resolvedCards.count) cards")
        return .success(resolvedCards)
    }
}
