import SwiftUI
import Combine

// MARK: - Debug Logging Helper

/// Debug print that only outputs in debug builds
private func debugPrint(_ items: Any..., separator: String = " ", terminator: String = "\n") {
    #if DEBUG
    print(items.map { "\($0)" }.joined(separator: separator), terminator: terminator)
    #endif
}

// Grid image data that maps each grid position to its specific URL (like desktop GUI)
struct GridImageData {
    let imageURL: String          // Specific front/back URL from expansion
    let cardName: String          // Display name (e.g., "kabira takedown" vs "kabira plateau")
    let setCode: String           // Set code for display
    let position: Int             // Grid position (0-8 per page)
    let page: Int                 // Page number
}

struct PrintSelectionView: View {
    @ObservedObject var resolvedCardsWrapper: ResolvedCardsWrapper  // Back to resolved cards for preview
    @Binding var decklistEntries: [DecklistEntryData]  // Source entries for updating print selections
    let onGeneratePDF: () -> Void
    let onDiscard: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var availablePrintings: [String: [CardPrintingData]] = [:] // Only for print selection modal
    @State private var isLoadingPrintings = false
    @State private var errorMessage: String?
    @State private var currentPage = 0

    // Image cache notification state
    @State private var imageCacheListenerID: UUID?
    @State private var refreshTrigger = false  // Used to force UI refresh when images are cached

    // Grid state computed dynamically from resolved cards (single source of truth)
    @State private var totalPages: Int = 1

    // Convenience property to access the resolved cards (for backward compatibility)
    private var resolvedCards: [ResolvedCard] {
        resolvedCardsWrapper.cards
    }
    
    var body: some View {
        VStack(spacing: 0) {
            if resolvedCards.isEmpty {
                Spacer()
                Text("No cards found in decklist")
                    .foregroundColor(.secondary)
                    .font(.subheadline)
                Spacer()
            } else {
                // 3x3 Grid - natural size only
                DynamicGridPreviewSection(
                    totalPages: totalPages,
                    currentPage: currentPage,
                    availablePrintings: availablePrintings,
                    decklistEntries: $decklistEntries,
                    resolvedCardsWrapper: resolvedCardsWrapper,
                    onPageChanged: { newPage in
                        print("📄 [PrintSelectionView] Page navigation: \(currentPage) -> \(newPage)")
                        currentPage = newPage
                    }
                )
                .id("\(refreshTrigger)-\(currentPage)") // Force re-render when refreshTrigger OR currentPage changes
                .frame(maxWidth: .infinity)
                
                // Spacer pushes buttons to bottom
                Spacer()
                
                // Bottom buttons - natural size at bottom
                HStack(spacing: 16) {
                    Button(action: {
                        onDiscard()
                        dismiss()
                    }) {
                        HStack {
                            Image(systemName: "xmark")
                            Text("Discard")
                        }
                        .foregroundColor(.red)
                        .padding(.vertical, 12)
                        .padding(.horizontal, 16)
                        .frame(maxWidth: .infinity)
                        .background(Color(UIColor.secondarySystemBackground))
                        .cornerRadius(10)
                        .overlay(
                            RoundedRectangle(cornerRadius: 10)
                                .stroke(Color.red, lineWidth: 1)
                        )
                    }
                    
                    Button(action: {
                        onGeneratePDF()
                        dismiss()
                    }) {
                        HStack {
                            Image(systemName: "doc.fill")
                            Text("Print")
                        }
                        .foregroundColor(.white)
                        .padding(.vertical, 12)
                        .padding(.horizontal, 16)
                        .frame(maxWidth: .infinity)
                        .background(Color.blue)
                        .cornerRadius(10)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(Color(UIColor.systemBackground))
            }
        }
        .navigationTitle("Preview")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            computeTotalPages()  // Compute page count from resolved cards
            loadAvailablePrintings()
            self.startWatchingImageCache()
        }
        .onDisappear {
            self.stopWatchingImageCache()
        }
    }

    // Compute total pages needed based on resolved cards (dynamic single source of truth)
    private func computeTotalPages() {
        guard !resolvedCards.isEmpty else {
            totalPages = 1
            return
        }

        print("🎯 [PrintSelectionView] Computing total pages from \(resolvedCards.count) resolved cards")

        // Count total images by expanding all resolved cards (same logic as PDF generation)
        var totalImages = 0
        for resolvedCard in resolvedCards {
            let imageUrls = ProxyGenerator.expandSingleCard(resolvedCard)
            totalImages += imageUrls.count
            print("🎯 [PrintSelectionView] \(resolvedCard.card.name) -> \(imageUrls.count) images")
        }

        self.totalPages = max(1, (totalImages + 8) / 9)  // Ceiling division (9 images per page)
        print("🎯 [PrintSelectionView] Total: \(totalImages) images across \(totalPages) pages")
    }

    // Helper function to determine display name based on position and card type
    private func getDisplayNameForPosition(_ index: Int, resolvedCard: ResolvedCard) -> String {
        // For the first image (index 0), always show the original card name
        if index == 0 {
            // For DFC cards, extract front face name
            if resolvedCard.card.name.contains(" // ") {
                let parts = resolvedCard.card.name.components(separatedBy: " // ")
                return parts[0]
            }
            return resolvedCard.card.name
        }

        // For the second image (index 1), determine based on card type
        if index == 1 {
            switch resolvedCard.card.backSideType {
            case .dfc:
                // For DFC cards, extract back face name
                if resolvedCard.card.name.contains(" // ") {
                    let parts = resolvedCard.card.name.components(separatedBy: " // ")
                    return parts.count > 1 ? parts[1] : resolvedCard.card.name
                }
                return resolvedCard.card.name
            case .meld:
                // For meld cards, use the meld result name
                return resolvedCard.card.backSideName ?? "Meld Result"
            case .none:
                return resolvedCard.card.name
            }
        }

        // For any additional images, use the original card name
        return resolvedCard.card.name
    }

    private func loadAvailablePrintings() {
        print("loadAvailablePrintings() called with \(resolvedCards.count) resolved cards")
        isLoadingPrintings = true
        errorMessage = nil
        
        // Get unique card names from resolved cards
        let uniqueCardNames = Set(resolvedCards.map { $0.card.name })
        print("Unique card names: \(uniqueCardNames)")
        
        Task {
            var newAvailablePrintings: [String: [CardPrintingData]] = [:]
            var hasError = false
            var errorMsg = ""
            
            for cardName in uniqueCardNames {
                print("Searching for printings of: \(cardName)")
                let result = ProxyGenerator.searchCardPrintings(cardName)
                
                switch result {
                case .success(let searchResult):
                    print("Found \(searchResult.cards.count) printings for \(cardName)")
                    newAvailablePrintings[cardName] = searchResult.cards
                case .failure(let error):
                    print("Error searching for \(cardName): \(error)")
                    hasError = true
                    errorMsg = "Failed to load printings for \(cardName): \(error.localizedDescription)"
                    break // Stop on first error to avoid spamming
                }
            }
            
            await MainActor.run {
                print("MainActor.run reached, hasError: \(hasError)")
                if hasError {
                    print("Setting error message: \(errorMsg)")
                    errorMessage = errorMsg
                } else {
                    print("Updating availablePrintings with \(newAvailablePrintings.count) cards")
                    for (cardName, printings) in newAvailablePrintings {
                        print("  \(cardName): \(printings.count) printings")
                    }
                    availablePrintings = newAvailablePrintings
                }
                
                isLoadingPrintings = false
                print("loadAvailablePrintings completed")
            }
        }
    }
    
    private func reloadGridImages() {
        // Force reload of all grid images by triggering a view refresh
        // This will cause all GridCardView instances to call loadImageForEntry again
        print("Reloading grid images...")
        
        // We can trigger this by updating a state variable that GridCardView observes
        // For now, just force the availablePrintings to refresh which will trigger image reloads
        let current = availablePrintings
        availablePrintings = [:]
        
        // Small delay then restore - this forces GridCardView to refresh
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            availablePrintings = current
        }
    }
    
    // MARK: - Image Cache Notifications
    
    private func startWatchingImageCache() {
        print("📡 [PrintSelectionView] Starting to watch image cache notifications")
        
        imageCacheListenerID = ProxyGenerator.startWatchingImageCache { [self] notification in
            self.handleImageCacheNotification(notification)
        }
        
        print("📝 [PrintSelectionView] Image cache listener registered with ID: \(imageCacheListenerID?.uuidString ?? "nil")")
    }
    
    private func stopWatchingImageCache() {
        if let listenerID = imageCacheListenerID {
            print("🛑 [PrintSelectionView] Stopping image cache watching for ID: \(listenerID)")
            ProxyGenerator.stopWatchingImageCache(listenerID: listenerID)
            imageCacheListenerID = nil
        }
    }
    
    private func handleImageCacheNotification(_ notification: ImageCacheChangeNotification) {
        print("🖼️ [PrintSelectionView] Received image cache notification: \(notification.changeType == 1 ? "CACHED" : "REMOVED") - \(notification.imageUrl)")

        // Only process ImageCached notifications (type 1)
        guard notification.changeType == 1 else { return }

        print("✅ [PrintSelectionView] Image cached: \(notification.imageUrl)")

        // Simple approach: refresh all grid views since they check cache on render
        refreshAllGridViews()
    }

    private func refreshAllGridViews() {
        print("🔄 [PrintSelectionView] Refreshing all grid views for image cache update")

        // Recompute total pages in case resolved cards changed (print selection)
        computeTotalPages()

        // Force SwiftUI to re-evaluate all grid views by toggling state
        refreshTrigger.toggle()

        // Also send a NotificationCenter notification as backup for any views that need it
        NotificationCenter.default.post(name: Notification.Name("ImageCacheUpdated"), object: nil)
    }
    
}


// MARK: - Dynamic Grid Preview Section (Single Source of Truth)

struct DynamicGridPreviewSection: View {
    let totalPages: Int
    let currentPage: Int
    let availablePrintings: [String: [CardPrintingData]]
    @Binding var decklistEntries: [DecklistEntryData]
    @ObservedObject var resolvedCardsWrapper: ResolvedCardsWrapper
    let onPageChanged: (Int) -> Void

    private let cardsPerPage = 9
    private let aspectRatio = 480.0/680.0

    var body: some View {
        VStack(spacing: 0) {
            // Grid layout (3x3 matching desktop app, no spacing like PDF) - takes most space
            GeometryReader { geometry in
                let availableWidth = geometry.size.width
                let availableHeight = geometry.size.height

                // Calculate the best fit for 3x3 grid with aspect ratio (GridView2 logic)
                let cellWidthFromWidth = availableWidth / 3.0
                let cellHeightFromWidth = cellWidthFromWidth / aspectRatio
                let totalHeightFromWidth = cellHeightFromWidth * 3.0

                let cellHeightFromHeight = availableHeight / 3.0
                let cellWidthFromHeight = cellHeightFromHeight * aspectRatio

                // Choose the constraint that fits
                let (cellWidth, cellHeight) = totalHeightFromWidth <= availableHeight
                    ? (cellWidthFromWidth, cellHeightFromWidth)
                    : (cellWidthFromHeight, cellHeightFromHeight)

                // Use calculated dimensions with fixed grid items
                let gridColumns = Array(repeating: GridItem(.fixed(cellWidth), spacing: 0), count: 3)

                LazyVGrid(columns: gridColumns, spacing: 0) {
                    ForEach(0..<cardsPerPage, id: \.self) { index in
                        DynamicGridCardView(
                            availablePrintings: availablePrintings,
                            currentPage: currentPage,
                            gridPosition: index,
                            decklistEntries: $decklistEntries,
                            resolvedCardsWrapper: resolvedCardsWrapper
                        )
                        .frame(width: cellWidth, height: cellHeight)
                        .id("\(resolvedCardsWrapper.cards.count)-\(currentPage)-\(index)")  // Force refresh when cards change
                    }
                }
                .id(resolvedCardsWrapper.cards.map { "\($0.card.name)-\($0.card.set)-\($0.card.language)" }.joined())  // Force grid refresh when any card changes
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .background(Color(UIColor.systemBackground))
            
            // Page navigation footer - takes fixed space
            if totalPages > 1 {
                HStack(spacing: 20) {
                    Button(action: {
                        if currentPage > 0 {
                            onPageChanged(currentPage - 1)
                        }
                    }) {
                        Image(systemName: "chevron.left.circle.fill")
                            .font(.title2)
                            .foregroundColor(currentPage > 0 ? .blue : .secondary)
                    }
                    .disabled(currentPage <= 0)
                    
                    Text("Page \(currentPage + 1) of \(totalPages)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Button(action: {
                        if currentPage < totalPages - 1 {
                            onPageChanged(currentPage + 1)
                        }
                    }) {
                        Image(systemName: "chevron.right.circle.fill")
                            .font(.title2)
                            .foregroundColor(currentPage < totalPages - 1 ? .blue : .secondary)
                    }
                    .disabled(currentPage >= totalPages - 1)
                }
                .padding(.vertical, 12)
                .background(Color(UIColor.systemBackground))
            } else {
                Text("Page 1 of 1")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .padding(.vertical, 12)
                    .background(Color(UIColor.systemBackground))
            }
        }
    }
}



// MARK: - Print Selection Modal (Desktop Pattern)

struct PrintSelectionModal: View {
    let cardName: String
    let availablePrintings: [CardPrintingData]
    let currentCard: CardPrintingData
    let onPrintingSelected: (CardPrintingData) -> Void
    
    @Environment(\.dismiss) private var dismiss
    
    private let gridColumns = Array(repeating: GridItem(.flexible(), spacing: 8), count: 4)
    
    var body: some View {
        NavigationView {
            VStack(spacing: 0) {
                // Header - fixed size
                VStack(alignment: .leading, spacing: 8) {
                    Text("Select Print for \(cardName)")
                        .font(.title2)
                        .fontWeight(.semibold)
                    
                    Text("Current: \(currentCard.set.uppercased()) • \(currentCard.language.uppercased())")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Text("\(availablePrintings.count) printings available")
                        .font(.caption)
                        .foregroundColor(.blue)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal)
                .padding(.vertical, 16)
                
                Divider()
                
                // Printings grid - takes remaining space and scrolls
                ScrollView {
                    LazyVGrid(columns: gridColumns, spacing: 12) {
                        ForEach(Array(availablePrintings.enumerated()), id: \.offset) { index, printing in
                            PrintingThumbnailView(
                                printing: printing,
                                isSelected: printing.set == currentCard.set && printing.language == currentCard.language,
                                onTap: {
                                    print("🎯 [PrintSelectionModal] onTap triggered for: \(printing.set) (\(printing.language))")
                                    onPrintingSelected(printing)
                                    print("🎯 [PrintSelectionModal] Called onPrintingSelected callback")
                                    dismiss()
                                    print("🎯 [PrintSelectionModal] Called dismiss()")
                                }
                            )
                        }
                    }
                    .padding()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .navigationTitle("Change Print")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
        .onAppear {
            print("🎯 [PrintSelectionModal] Modal appeared for: \(cardName) with \(availablePrintings.count) printings")
        }
    }
}

struct PrintingThumbnailView: View {
    let printing: CardPrintingData
    let isSelected: Bool
    let onTap: () -> Void
    
    @State private var imageData: Data?
    @State private var isLoadingImage = false
    
    var body: some View {
        VStack(spacing: 4) {
            // Thumbnail image
            ZStack {
                Rectangle()
                    .fill(Color(UIColor.systemGray6))
                    .aspectRatio(480.0/680.0, contentMode: .fit)
                
                if let imageData = imageData, let uiImage = UIImage(data: imageData) {
                    // Show cached image
                    Image(uiImage: uiImage)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                } else if isLoadingImage {
                    // Loading state
                    ProgressView()
                        .scaleEffect(0.6)
                } else {
                    // Placeholder
                    VStack(spacing: 2) {
                        Image(systemName: "photo")
                            .foregroundColor(.secondary)
                            .font(.caption)
                        Text("Loading")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? Color.blue : Color.clear, lineWidth: 3)
            )
            
            // Set and language info
            VStack(spacing: 1) {
                Text(printing.set.uppercased())
                    .font(.caption2)
                    .fontWeight(.medium)
                    .foregroundColor(.primary)
                
                Text(printing.language.uppercased())
                    .font(.caption2)
                    .foregroundColor(.secondary)
            }
        }
        .contentShape(Rectangle()) // Make entire view tappable
        .onTapGesture {
            print("🖱️ [PrintingThumbnailView] Tapped on printing: \(printing.set) (\(printing.language))")
            onTap()
        }
        .onAppear {
            loadImage()
        }
    }
    
    private func loadImage() {
        // Get primary image URL for this printing
        let imageURL = printing.borderCropURL
        
        // Check if image is cached
        guard ProxyGenerator.initialize() else {
            print("Failed to initialize ProxyGenerator for thumbnail loading")
            return
        }
        
        switch ProxyGenerator.getCachedImageData(for: imageURL) {
        case .success(let data):
            print("✅ [PrintingThumbnailView] Using cached image for: \(printing.set)")
            imageData = data
            isLoadingImage = false
        case .failure:
            print("🔍 [PrintingThumbnailView] Image not cached yet for: \(printing.set)")
            imageData = nil
            isLoadingImage = true
        }
    }
}

// Dynamic GridCardView that computes everything from resolved cards (single source of truth)
struct DynamicGridCardView: View {
    let availablePrintings: [String: [CardPrintingData]]
    let currentPage: Int
    let gridPosition: Int  // Position on the current page (0-8)
    @Binding var decklistEntries: [DecklistEntryData]
    @ObservedObject var resolvedCardsWrapper: ResolvedCardsWrapper

    @State private var imageData: Data?
    @State private var isLoadingImage = false
    @State private var showPrintSelection = false

    // Computed properties that dynamically determine what to show for this grid position
    private var gridData: (resolvedCard: ResolvedCard, imageURL: String, displayName: String)? {
        // Calculate the absolute position across all pages
        let absolutePosition = currentPage * 9 + gridPosition

        // Walk through resolved cards and their expansions to find the card for this position
        var currentImageIndex = 0

        for resolvedCard in resolvedCardsWrapper.cards {
            let imageUrls = ProxyGenerator.expandSingleCard(resolvedCard)

            // Check if this card's images include the position we want
            if absolutePosition >= currentImageIndex && absolutePosition < currentImageIndex + imageUrls.count {
                let relativeIndex = absolutePosition - currentImageIndex
                let imageURL = imageUrls[relativeIndex]
                let displayName = getDisplayNameForPosition(relativeIndex, resolvedCard: resolvedCard)

                return (resolvedCard, imageURL, displayName)
            }

            currentImageIndex += imageUrls.count
        }

        return nil // This grid position is empty
    }

    // Helper function to determine display name based on position and card type
    private func getDisplayNameForPosition(_ index: Int, resolvedCard: ResolvedCard) -> String {
        // For the first image (index 0), always show the original card name
        if index == 0 {
            // For DFC cards, extract front face name
            if resolvedCard.card.name.contains(" // ") {
                let parts = resolvedCard.card.name.components(separatedBy: " // ")
                return parts[0]
            }
            return resolvedCard.card.name
        }

        // For the second image (index 1), determine based on card type
        if index == 1 {
            switch resolvedCard.card.backSideType {
            case .dfc:
                // For DFC cards, extract back face name
                if resolvedCard.card.name.contains(" // ") {
                    let parts = resolvedCard.card.name.components(separatedBy: " // ")
                    return parts.count > 1 ? parts[1] : resolvedCard.card.name
                }
                return resolvedCard.card.name
            case .meld:
                // For meld cards, use the meld result name
                return resolvedCard.card.backSideName ?? "Meld Result"
            case .none:
                return resolvedCard.card.name
            }
        }

        // For any additional images, use the original card name
        return resolvedCard.card.name
    }

    var body: some View {
        ZStack {
            // Background (no borders in PDF)
            Rectangle()
                .fill(Color(UIColor.systemBackground))

            if let (resolvedCard, _, displayName) = gridData {
                if let imageData = imageData, let uiImage = UIImage(data: imageData) {
                    // Show specific image from URL
                    Image(uiImage: uiImage)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                } else if isLoadingImage {
                    // Loading state
                    VStack(spacing: 4) {
                        ProgressView()
                            .scaleEffect(0.8)
                        Text("Loading...")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                } else {
                    // Placeholder with correct card name
                    VStack(spacing: 4) {
                        Image(systemName: "photo")
                            .foregroundColor(.secondary)
                            .font(.title3)

                        Text(displayName)  // Uses dynamically computed name
                            .font(.caption2)
                            .multilineTextAlignment(.center)
                            .foregroundColor(.primary)
                            .lineLimit(3)
                            .padding(.horizontal, 4)

                        Text("[\(resolvedCard.card.set.uppercased())]")
                            .font(.caption2)
                            .foregroundColor(.blue)
                    }
                }
            } else {
                // Empty grid position (no borders like PDF)
                Rectangle()
                    .fill(Color(UIColor.systemBackground))
            }
        }
        .onTapGesture {
            if gridData != nil {
                showPrintSelection = true
            }
        }
        .onAppear {
            loadImageForCurrentCard()
        }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("ImageCacheUpdated"))) { _ in
            // Reload when print selection changes or images are cached
            loadImageForCurrentCard()
        }
        .fullScreenCover(isPresented: $showPrintSelection) {
            if let (resolvedCard, _, displayName) = gridData,
               let cardPrintings = availablePrintings[resolvedCard.card.name] {

                PrintSelectionModal(
                    cardName: displayName,
                    availablePrintings: cardPrintings,
                    currentCard: resolvedCard.card,
                    onPrintingSelected: { selectedPrinting in
                        debugPrint("✅ [DynamicGridCardView] Selected new printing: \(selectedPrinting.set) (\(selectedPrinting.language)) for \(displayName)")

                        // Find the decklist entry index for this resolved card
                        if let decklistEntryIndex = resolvedCardsWrapper.cards.firstIndex(where: { $0 === resolvedCard }) {
                            debugPrint("🔥 [DynamicGridCardView] About to call updatePrintSelection for decklistEntryIndex=\(decklistEntryIndex)")
                            // Desktop pattern: Update source DecklistEntryData and re-resolve everything
                            updatePrintSelection(decklistEntryIndex: decklistEntryIndex,
                                               newPrinting: selectedPrinting)
                        } else {
                            print("❌ [DynamicGridCardView] Could not find decklist entry index for resolved card")
                        }

                        debugPrint("🔥 [DynamicGridCardView] updatePrintSelection call completed")
                        showPrintSelection = false
                    }
                )
            }
        }
    }

    private func loadImageForCurrentCard() {
        guard let (_, imageURL, displayName) = gridData else {
            // Clear image data for empty grid positions
            imageData = nil
            isLoadingImage = false
            return
        }

        print("🖼️ [DynamicGridCardView] Loading current image for \(displayName): \(imageURL)")

        // Check if image is already cached
        guard ProxyGenerator.initialize() else {
            print("Failed to initialize ProxyGenerator for image loading")
            return
        }

        switch ProxyGenerator.getCachedImageData(for: imageURL) {
        case .success(let data):
            print("✅ [DynamicGridCardView] Using cached image for URL: \(imageURL)")
            imageData = data
            isLoadingImage = false
        case .failure:
            print("🔍 [DynamicGridCardView] Image not cached yet for URL: \(imageURL)")
            imageData = nil
            isLoadingImage = true
        }
    }

    // Update print selection following desktop GUI pattern
    private func updatePrintSelection(decklistEntryIndex: Int, newPrinting: CardPrintingData) {
        print("🔄 [DynamicGridCardView] Updating print selection for decklist entry \(decklistEntryIndex) to \(newPrinting.set) (\(newPrinting.language))")

        // Validate index bounds
        guard decklistEntryIndex >= 0 && decklistEntryIndex < decklistEntries.count else {
            print("🔄 [DynamicGridCardView] ❌ Invalid decklist entry index: \(decklistEntryIndex) (count: \(decklistEntries.count))")
            return
        }

        let entry = decklistEntries[decklistEntryIndex]
        print("🔄 [DynamicGridCardView] Before update: '\(entry.name)' set='\(entry.set ?? "nil")', lang='\(entry.language ?? "nil")'")

        // Update the source entry with new set/language (desktop pattern)
        decklistEntries[decklistEntryIndex].set = newPrinting.set
        decklistEntries[decklistEntryIndex].language = newPrinting.language

        print("🔄 [DynamicGridCardView] After update: set='\(decklistEntries[decklistEntryIndex].set ?? "nil")', lang='\(decklistEntries[decklistEntryIndex].language ?? "nil")'")

        // Re-resolve all cards from updated entries to get meld consistency
        reResolveAllCards()
    }

    // Re-resolve all cards from updated decklist entries (handles meld consistency automatically)
    private func reResolveAllCards() {
        print("🔄 [DynamicGridCardView] Re-resolving all cards from updated decklist entries")
        print("🔄 [DynamicGridCardView] Current resolved cards count: \(resolvedCardsWrapper.cards.count)")

        // Log the current state of decklist entries
        for (index, entry) in decklistEntries.enumerated() {
            print("🔄 [DynamicGridCardView] Entry \(index): '\(entry.name)' set='\(entry.set ?? "nil")' lang='\(entry.language ?? "nil")'")
        }

        // Use the Rust core to resolve cards from the updated entries
        let result = ProxyGenerator.resolveEntriesToCards(decklistEntries)

        switch result {
        case .success(let newResolvedCards):
            print("✅ [DynamicGridCardView] Successfully re-resolved \(newResolvedCards.count) cards")

            // Log some details about the new resolved cards
            for (index, card) in newResolvedCards.enumerated() {
                print("✅ [DynamicGridCardView] New card \(index): '\(card.card.name)' set='\(card.card.set)' lang='\(card.card.language)'")
            }

            // Update the resolved cards wrapper (this will trigger UI update)
            print("🔄 [DynamicGridCardView] Updating resolvedCardsWrapper.cards...")
            resolvedCardsWrapper.cards = newResolvedCards
            print("🔄 [DynamicGridCardView] Updated resolvedCardsWrapper.cards to \(resolvedCardsWrapper.cards.count) cards")

            // Force manual refresh of all grid views
            NotificationCenter.default.post(name: Notification.Name("ImageCacheUpdated"), object: nil)
            print("📡 [DynamicGridCardView] Sent manual refresh notification")

        case .failure(let error):
            print("❌ [DynamicGridCardView] Failed to re-resolve cards: \(error)")
        }
    }
}

#Preview {
    PrintSelectionView(
        resolvedCardsWrapper: ResolvedCardsWrapper(cards: []),
        decklistEntries: .constant([]),
        onGeneratePDF: {},
        onDiscard: {}
    )
}
