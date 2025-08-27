import SwiftUI
import Combine

struct PrintSelectionView: View {
    @ObservedObject var resolvedCardsWrapper: ResolvedCardsWrapper  // Wraps the array of ResolvedCard objects
    let onGeneratePDF: () -> Void
    
    @Environment(\.dismiss) private var dismiss
    @State private var availablePrintings: [String: [CardPrintingData]] = [:] // Only for print selection modal
    @State private var isLoadingPrintings = false
    @State private var errorMessage: String?
    @State private var showCardList = false
    
    // Image cache notification state
    @State private var imageCacheListenerID: UUID?
    
    // Convenience property to access the resolved cards
    private var resolvedCards: [ResolvedCard] {
        resolvedCardsWrapper.cards
    }
    
    var body: some View {
        NavigationView {
            VStack(spacing: 16) {
                if resolvedCardsWrapper.cards.isEmpty {
                    Text("No cards found in decklist")
                        .foregroundColor(.secondary)
                        .font(.subheadline)
                } else {
                    // Header info
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Preview & Print Selection")
                            .font(.title2)
                            .fontWeight(.semibold)
                        
                        let totalCards = resolvedCards.reduce(0) { $0 + Int($1.quantity) }
                        Text("\(resolvedCards.count) unique cards, \(totalCards) total")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        
                        if isLoadingPrintings {
                            HStack {
                                ProgressView()
                                    .scaleEffect(0.8)
                                Text("Loading available printings...")
                                    .font(.caption2)
                                    .foregroundColor(.secondary)
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal)
                    
                    // Background loading happens automatically - no progress indicator needed
                    
                    // 3x3 Grid Preview (matching desktop app)
                    VStack(spacing: 12) {
                        HStack {
                            Text("Preview Grid")
                                .font(.headline)
                            
                            Spacer()
                            
                            Button("Reload Images") {
                                reloadGridImages()
                            }
                            .font(.caption)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Color.blue)
                            .foregroundColor(.white)
                            .cornerRadius(8)
                        }
                        .padding(.horizontal)
                        
                        GridPreviewSection(
                            resolvedCards: resolvedCards,
                            currentPage: 0 // TODO: Add page navigation
                        )
                        .padding(.horizontal)
                        
                        // Card Selection List (collapsible) - matches desktop functionality
                        VStack(spacing: 8) {
                            HStack {
                                Text("Card Selection")
                                    .font(.headline)
                                Spacer()
                                Button(showCardList ? "Hide" : "Show") {
                                    withAnimation {
                                        showCardList.toggle()
                                    }
                                }
                                .font(.caption)
                                .foregroundColor(.blue)
                            }
                            .padding(.horizontal)
                            
                            if showCardList {
                                ScrollView {
                                    LazyVStack(spacing: 12) {
                                        ForEach(Array(resolvedCards.enumerated()), id: \.offset) { index, resolvedCard in
                                            ResolvedCardRow(
                                                resolvedCard: resolvedCard,
                                                availablePrintings: availablePrintings[resolvedCard.card.name] ?? []
                                            )
                                        }
                                    }
                                    .padding(.horizontal)
                                }
                                .frame(maxHeight: 300)
                            }
                        }
                    }
                    
                    if let errorMessage = errorMessage {
                        Text(errorMessage)
                            .foregroundColor(.red)
                            .font(.caption)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }
                    
                    // Generate PDF button
                    Button(action: {
                        onGeneratePDF()
                    }) {
                        HStack {
                            Image(systemName: "doc.fill")
                            Text("Generate PDF with Selected Prints")
                        }
                        .foregroundColor(.white)
                        .padding()
                        .frame(maxWidth: .infinity)
                        .background(Color.blue)
                        .cornerRadius(10)
                    }
                    .padding(.horizontal)
                }
            }
            .padding(.vertical)
            .navigationTitle("Print Selection")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
            .onAppear {
                loadAvailablePrintings()
                self.startWatchingImageCache()
            }
            .onDisappear {
                self.stopWatchingImageCache()
            }
        }
    }
    
    private func entryKey(for entry: DecklistEntryData) -> String {
        // Use source line number if available, otherwise use name
        if let lineNumber = entry.sourceLineNumber {
            return "line_\(lineNumber)"
        } else {
            return "name_\(entry.name)"
        }
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
                    
                    // Initialize selected printings with best matches
                    initializeSelectedPrintings()
                }
                
                isLoadingPrintings = false
                print("loadAvailablePrintings completed")
            }
        }
    }
    
    // Legacy function removed - desktop pattern doesn't need complex matching
    private func initializeSelectedPrintings() {
        // No-op: Resolved cards are the source of truth, no matching needed
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
    
    // Legacy function removed - desktop pattern modifies resolved cards directly
    private func applySelectedPrintingToEntry(entryIndex: Int, printingIndex: Int) {
        // No-op: Print selection modifies ResolvedCard.card directly
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
        
        // Smart approach: find which specific cards use this URL and refresh only those
        refreshCardsUsingImageURL(notification.imageUrl)
    }
    
    private func refreshCardsUsingImageURL(_ cachedURL: String) {
        // Check which ResolvedCard objects use this specific URL (desktop pattern)
        var affectedCardNames: Set<String> = []
        
        for resolvedCard in resolvedCards {
            let imageUrls = resolvedCard.getImageUrls()
            if imageUrls.contains(cachedURL) {
                affectedCardNames.insert(resolvedCard.card.name)
                print("🎯 [PrintSelectionView] Found cached image for resolved card: \(resolvedCard.card.name)")
            }
        }
        
        guard !affectedCardNames.isEmpty else {
            print("ℹ️ [PrintSelectionView] Cached URL not found in any available printings: \(cachedURL)")
            return
        }
        
        print("🔄 [PrintSelectionView] Triggering targeted refresh for \(affectedCardNames.count) affected cards")
        
        // Trigger SwiftUI refresh for affected ResolvedCard objects (desktop pattern)
        for resolvedCard in resolvedCards {
            if affectedCardNames.contains(resolvedCard.card.name) {
                print("🔄 [PrintSelectionView] Refreshing UI for resolved card: \(resolvedCard.card.name)")
                // Force SwiftUI to re-evaluate this ResolvedCard's views
                resolvedCard.objectWillChange.send()
            }
        }
        
        // Also send a NotificationCenter notification as backup
        NotificationCenter.default.post(name: Notification.Name("ImageCacheUpdated"), object: nil)
    }
    
}

struct CardEntryRow: View {
    let entry: DecklistEntryData
    let selectedPrintingIndex: Int
    let availablePrintings: [CardPrintingData]
    let onPrintingSelected: (Int) -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Card info header
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(entry.multiple)x \(entry.name)")
                        .font(.subheadline)
                        .fontWeight(.medium)
                    
                    HStack {
                        if let set = entry.set {
                            Text("[\(set.uppercased())]")
                                .font(.caption)
                                .foregroundColor(.blue)
                        }
                        if let language = entry.language {
                            Text("[\(language.uppercased())]")
                                .font(.caption)
                                .foregroundColor(.green)
                        }
                        Text(entry.faceMode.displayName)
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                }
                
                Spacer()
                
                // Show loading state or print count
                if availablePrintings.isEmpty {
                    Text("Loading printings...")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else {
                    Text("\(availablePrintings.count) printings")
                        .font(.caption)
                        .foregroundColor(.blue)
                }
            }
            
            // Available printings (simple text list for now)
            if !availablePrintings.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Available printings:")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    ForEach(Array(availablePrintings.enumerated()), id: \.offset) { index, printing in
                        HStack {
                            Button(action: {
                                onPrintingSelected(index)
                            }) {
                                HStack {
                                    Image(systemName: selectedPrintingIndex == index ? "checkmark.circle.fill" : "circle")
                                        .foregroundColor(selectedPrintingIndex == index ? .blue : .gray)
                                    
                                    Text("\(printing.set.uppercased()) • \(printing.language.uppercased())")
                                        .font(.caption)
                                        .foregroundColor(.primary)
                                    
                                    Spacer()
                                }
                            }
                            .buttonStyle(PlainButtonStyle())
                        }
                    }
                }
                .padding(.leading, 16)
            }
        }
        .padding()
        .background(Color(UIColor.secondarySystemBackground))
        .cornerRadius(8)
    }
}

// MARK: - Grid Preview Section

struct GridPreviewSection: View {
    let resolvedCards: [ResolvedCard]
    let currentPage: Int
    
    private let gridColumns = Array(repeating: GridItem(.flexible(), spacing: 4), count: 3)
    private let cardsPerPage = 9
    
    var body: some View {
        VStack(spacing: 8) {
            // Grid layout (3x3 matching desktop app)
            LazyVGrid(columns: gridColumns, spacing: 4) {
                ForEach(0..<cardsPerPage, id: \.self) { index in
                    GridCardView(
                        resolvedCard: getResolvedCardForGridPosition(index)
                    )
                    .aspectRatio(480.0/680.0, contentMode: .fit) // Magic card aspect ratio
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .padding(8)
            .background(Color(UIColor.systemBackground))
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color(UIColor.separator), lineWidth: 1)
            )
            
            // Page info (for future multi-page support)
            Text("Page \(currentPage + 1) of 1")
                .font(.caption)
                .foregroundColor(.secondary)
        }
    }
    
    // Legacy function - not needed with ResolvedCard architecture
    private func getEntryForGridPosition(_ position: Int) -> DecklistEntryData? {
        return nil // No longer used with ResolvedCard architecture
    }
    
    private func getResolvedCardForGridPosition(_ position: Int) -> ResolvedCard? {
        // Expand resolved cards based on quantity (e.g., 4x Lightning Bolt = 4 grid positions)
        let expandedCards = resolvedCards.flatMap { resolvedCard in
            Array(repeating: resolvedCard, count: Int(resolvedCard.quantity))
        }
        
        let startIndex = currentPage * cardsPerPage
        let targetIndex = startIndex + position
        
        return targetIndex < expandedCards.count ? expandedCards[targetIndex] : nil
    }
}

struct GridCardView: View {
    let resolvedCard: ResolvedCard?  // The resolved card to display
    
    @State private var imageData: Data?
    @State private var isLoadingImage = false
    
    var body: some View {
        ZStack {
            // Background
            RoundedRectangle(cornerRadius: 6)
                .fill(Color(UIColor.secondarySystemBackground))
                .overlay(
                    RoundedRectangle(cornerRadius: 6)
                        .stroke(Color(UIColor.separator), lineWidth: 1)
                )
            
            if let resolvedCard = resolvedCard {
                if let imageData = imageData, let uiImage = UIImage(data: imageData) {
                    // Show cached image
                    Image(uiImage: uiImage)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .cornerRadius(6)
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
                // Empty grid position
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color.clear)
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
        .onChange(of: resolvedCard?.card) { _ in
            if let resolvedCard = resolvedCard {
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
        print("Loading image for resolved card: \(resolvedCard.card.name)")
        
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
    
    // Legacy function - no longer used with ResolvedCard architecture
    private func loadImageForEntry(_ entry: DecklistEntryData) {
        // No-op: ResolvedCard architecture uses loadImageForResolvedCard instead
    }
    
    private func tryLoadAnyCachedImageForCard(_ cardName: String) {
        // For now, just return nil - this will show placeholder
        // The real fix is to ensure availablePrintings gets populated quickly
        // Once availablePrintings is loaded, the onChange handler will trigger loadImageForEntry again
        imageData = nil
        isLoadingImage = false
    }
    
    private func entryKey(for entry: DecklistEntryData) -> String {
        // Use source line number if available, otherwise use name
        if let lineNumber = entry.sourceLineNumber {
            return "line_\(lineNumber)"
        } else {
            return "name_\(entry.name)"
        }
    }
}

// MARK: - ResolvedCardRow (Desktop Pattern)

struct ResolvedCardRow: View {
    @ObservedObject var resolvedCard: ResolvedCard
    let availablePrintings: [CardPrintingData]
    
    var body: some View {
        HStack {
            // Card info
            VStack(alignment: .leading, spacing: 4) {
                Text(resolvedCard.card.name)
                    .font(.headline)
                
                Text("\(resolvedCard.quantity)x • \(resolvedCard.card.set.uppercased()) • \(resolvedCard.faceMode.displayName)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            // Print selection button (if alternatives exist)
            if availablePrintings.count > 1 {
                Button("Change Print") {
                    // TODO: Show print selection modal that modifies resolvedCard.card directly
                }
                .buttonStyle(.bordered)
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .background(Color(UIColor.secondarySystemBackground))
        .cornerRadius(8)
    }
}

#Preview {
    PrintSelectionView(
        resolvedCardsWrapper: ResolvedCardsWrapper(cards: []),
        onGeneratePDF: {}
    )
}