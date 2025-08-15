import Foundation

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
        
        let result = proxy_initialize()
        isInitialized = (result == 0)
        
        if !isInitialized {
            print("ProxyGenerator initialization failed with code: \(result)")
        }
        
        return isInitialized
    }
    
    /// Simple test function to verify FFI is working
    static func testConnection() -> Int32 {
        return proxy_test_connection()
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
        let result = proxy_generate_pdf_from_decklist(
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
        defer { proxy_free_buffer(buffer) }
        
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
        let messagePtr = proxy_get_error_message(errorCode)
        guard let messagePtr = messagePtr else {
            return "Unknown error"
        }
        // Note: proxy_get_error_message returns static strings - no need to free
        return String(cString: messagePtr)
    }
}