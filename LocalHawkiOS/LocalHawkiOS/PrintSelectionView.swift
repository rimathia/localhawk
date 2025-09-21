import SwiftUI
import Combine

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

    // NEW: Grid image data computed from expansion (replaces ResolvedCard approach)
    @State private var gridImageData: [GridImageData] = []
    @State private var totalPages: Int = 1

    // Convenience property to access the resolved cards (for backward compatibility)
    private var resolvedCards: [ResolvedCard] {
        resolvedCardsWrapper.cards
    }
    
    var body: some View {
        VStack(spacing: 0) {
            if gridImageData.isEmpty {
                Spacer()
                Text("No cards found in decklist")
                    .foregroundColor(.secondary)
                    .font(.subheadline)
                Spacer()
            } else {
                // 3x3 Grid - natural size only
                GridPreviewSection(
                    gridImageData: gridImageData,
                    totalPages: totalPages,
                    currentPage: currentPage,
                    availablePrintings: availablePrintings,
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
            computeGridImageData()  // NEW: Compute grid data first
            loadAvailablePrintings()
            self.startWatchingImageCache()
        }
        .onDisappear {
            self.stopWatchingImageCache()
        }
    }

    // NEW: Compute grid image data from resolved cards using expansion (like desktop GUI)
    private func computeGridImageData() {
        guard !resolvedCards.isEmpty else {
            gridImageData = []
            totalPages = 1
            return
        }

        print("🎯 [PrintSelectionView] Computing grid image data from \(resolvedCards.count) resolved cards")

        var allImageData: [GridImageData] = []
        var currentPosition = 0

        for resolvedCard in resolvedCards {
            print("🎯 [PrintSelectionView] Processing: \(resolvedCard.card.name) qty=\(resolvedCard.quantity) face_mode=\(resolvedCard.faceMode)")

            // Use expansion logic to get specific URLs (like desktop GUI)
            let imageUrls = ProxyGenerator.expandSingleCard(resolvedCard)

            print("🎯 [PrintSelectionView] Expansion result: \(imageUrls.count) URLs")

            // Create GridImageData for each URL
            for (index, imageUrl) in imageUrls.enumerated() {
                let page = currentPosition / 9  // 9 images per page (3x3 grid)
                let positionInPage = currentPosition % 9

                // Determine display name based on position and card type
                let displayName = getDisplayNameForPosition(index, resolvedCard: resolvedCard)

                let gridImage = GridImageData(
                    imageURL: imageUrl,
                    cardName: displayName,
                    setCode: resolvedCard.card.set,
                    position: positionInPage,
                    page: page
                )

                allImageData.append(gridImage)
                currentPosition += 1

                print("  [\(index)] \(displayName) -> page=\(page) pos=\(positionInPage)")
            }
        }

        self.gridImageData = allImageData
        self.totalPages = max(1, (allImageData.count + 8) / 9)  // Ceiling division
        print("🎯 [PrintSelectionView] Generated \(allImageData.count) grid images across \(totalPages) pages")

        // Debug: Show page distribution
        for page in 0..<totalPages {
            let imagesOnPage = allImageData.filter { $0.page == page }
            print("📄 [PrintSelectionView] Page \(page): \(imagesOnPage.count) images")
            for (index, imageData) in imagesOnPage.enumerated() {
                if index < 5 { // Show first 5 per page
                    print("    [\(imageData.position)] \(imageData.cardName)")
                }
            }
        }
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

        // Force SwiftUI to re-evaluate all grid views by toggling state
        // Since NewGridCardView checks the cache when it renders, this will pick up new images
        refreshTrigger.toggle()

        // Also send a NotificationCenter notification as backup for any views that need it
        NotificationCenter.default.post(name: Notification.Name("ImageCacheUpdated"), object: nil)
    }
    
}


// MARK: - Grid Preview Section

struct GridPreviewSection: View {
    let gridImageData: [GridImageData]
    let totalPages: Int
    let currentPage: Int
    let availablePrintings: [String: [CardPrintingData]]
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
                        NewGridCardView(
                            gridImageData: getGridImageDataForPosition(index),
                            availablePrintings: availablePrintings
                        )
                        .frame(width: cellWidth, height: cellHeight)
                    }
                }
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
    
    private func getGridImageDataForPosition(_ position: Int) -> GridImageData? {
        // Find the grid image data for this position on the current page
        return gridImageData.first { imageData in
            imageData.page == currentPage && imageData.position == position
        }
    }
}

struct GridCardView: View {
    let resolvedCard: ResolvedCard?  // The resolved card to display
    let availablePrintings: [String: [CardPrintingData]] // Available printings for modal
    
    @State private var imageData: Data?
    @State private var isLoadingImage = false
    @State private var showPrintSelection = false
    
    var body: some View {
        ZStack {
            // Background (no borders in PDF)
            Rectangle()
                .fill(Color(UIColor.systemBackground))
            
            if let resolvedCard = resolvedCard {
                if let imageData = imageData, let uiImage = UIImage(data: imageData) {
                    // Show cached image
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
                    // Placeholder with card name
                    VStack(spacing: 4) {
                        Image(systemName: "photo")
                            .foregroundColor(.secondary)
                            .font(.title3)
                        
                        Text(resolvedCard.card.name)
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
                    .fill(Color.clear)
            }
        }
        .onTapGesture {
            if let resolvedCard = resolvedCard, 
               let cardPrintings = availablePrintings[resolvedCard.card.name],
               cardPrintings.count > 1 {
                showPrintSelection = true
            }
        }
        .fullScreenCover(isPresented: $showPrintSelection) {
            if let resolvedCard = resolvedCard,
               let cardPrintings = availablePrintings[resolvedCard.card.name] {
                PrintSelectionModal(
                    cardName: resolvedCard.card.name,
                    availablePrintings: cardPrintings,
                    currentCard: resolvedCard.card,
                    onPrintingSelected: { selectedCard in
                        print("✅ [PrintSelectionModal] Selected new printing: \(selectedCard.set) (\(selectedCard.language)) for \(resolvedCard.card.name)")
                        print("🔍 [PrintSelectionModal] Before change: \(resolvedCard.card.set) (\(resolvedCard.card.language))")
                        // Modify card object in place (desktop pattern)
                        resolvedCard.card = selectedCard
                        print("🔍 [PrintSelectionModal] After change: \(resolvedCard.card.set) (\(resolvedCard.card.language))")
                        resolvedCard.objectWillChange.send()
                        print("🔄 [PrintSelectionModal] Updated resolvedCard and sent objectWillChange notification")
                        
                        // Force manual refresh of all grid views for this card
                        NotificationCenter.default.post(name: Notification.Name("ImageCacheUpdated"), object: nil)
                        print("📡 [PrintSelectionModal] Sent manual refresh notification")
                    }
                )
            }
        }
        .onAppear {
            if let resolvedCard = resolvedCard {
                print("GridCardView onAppear for: \(resolvedCard.card.name)")
                loadImageForResolvedCard(resolvedCard)
            } else {
                print("GridCardView onAppear with nil resolved card")
            }
        }
        .onChange(of: resolvedCard?.card.set) { _ in
            if let resolvedCard = resolvedCard {
                print("🔄 [GridCardView] Detected card set change for: \(resolvedCard.card.name) -> \(resolvedCard.card.set) (\(resolvedCard.card.language))")
                loadImageForResolvedCard(resolvedCard)
            }
        }
        .onChange(of: resolvedCard?.card.language) { _ in
            if let resolvedCard = resolvedCard {
                print("🔄 [GridCardView] Detected card language change for: \(resolvedCard.card.name) -> \(resolvedCard.card.set) (\(resolvedCard.card.language))")
                loadImageForResolvedCard(resolvedCard)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: Notification.Name("ImageCacheUpdated"))) { _ in
            if let resolvedCard = resolvedCard {
                print("🔄 [GridCardView] Manual refresh notification received for: \(resolvedCard.card.name)")
                loadImageForResolvedCard(resolvedCard)
            }
        }
    }
    
    private func loadImageForResolvedCard(_ resolvedCard: ResolvedCard) {
        print("🖼️ [GridCardView] Loading image for resolved card: \(resolvedCard.card.name) (\(resolvedCard.card.set))")
        
        // Get the primary image URL for this resolved card based on face mode
        let imageUrls = resolvedCard.getImageUrls()
        guard let primaryImageURL = imageUrls.first else {
            print("No image URL available for resolved card: \(resolvedCard.card.name)")
            return
        }
        
        // Check if image is already cached
        guard ProxyGenerator.initialize() else {
            print("Failed to initialize ProxyGenerator for image loading")
            return
        }
        
        switch ProxyGenerator.getCachedImageData(for: primaryImageURL) {
        case .success(let data):
            print("✅ [GridCardView] Using cached image for: \(resolvedCard.card.name)")
            imageData = data
            isLoadingImage = false
        case .failure:
            print("🔍 [GridCardView] Image not cached yet for URL: \(primaryImageURL)")
            imageData = nil
            isLoadingImage = true
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
                                    onPrintingSelected(printing)
                                    dismiss()
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

// NEW: GridCardView that uses specific image URLs (like desktop GUI)
struct NewGridCardView: View {
    let gridImageData: GridImageData?  // Specific image data for this position
    let availablePrintings: [String: [CardPrintingData]] // Available printings for modal

    @State private var imageData: Data?
    @State private var isLoadingImage = false
    @State private var showPrintSelection = false

    var body: some View {
        ZStack {
            // Background (no borders in PDF)
            Rectangle()
                .fill(Color(UIColor.systemBackground))

            if let gridImage = gridImageData {
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

                        Text(gridImage.cardName)  // Uses correct name (takedown vs plateau)
                            .font(.caption2)
                            .multilineTextAlignment(.center)
                            .foregroundColor(.primary)
                            .lineLimit(3)
                            .padding(.horizontal, 4)

                        Text("[\(gridImage.setCode.uppercased())]")
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
            // TODO: Add print selection modal support if needed
        }
        .onAppear {
            if let gridImage = gridImageData {
                print("🎯 [NewGridCardView] Loading specific image: \(gridImage.cardName) -> \(gridImage.imageURL)")
                loadSpecificImage(gridImage.imageURL)
            }
        }
    }

    private func loadSpecificImage(_ imageURL: String) {
        print("🖼️ [NewGridCardView] Loading specific URL: \(imageURL)")

        // Check if image is already cached
        guard ProxyGenerator.initialize() else {
            print("Failed to initialize ProxyGenerator for image loading")
            return
        }

        switch ProxyGenerator.getCachedImageData(for: imageURL) {
        case .success(let data):
            print("✅ [NewGridCardView] Using cached image for URL: \(imageURL)")
            imageData = data
            isLoadingImage = false
        case .failure:
            print("🔍 [NewGridCardView] Image not cached yet for URL: \(imageURL)")
            imageData = nil
            isLoadingImage = true
        }
    }
}

#Preview {
    PrintSelectionView(
        resolvedCardsWrapper: ResolvedCardsWrapper(cards: []),
        onGeneratePDF: {},
        onDiscard: {}
    )
}
