import SwiftUI

struct PrintSelectionView: View {
    let entries: [DecklistEntryData]
    let onGeneratePDF: () -> Void
    
    @Environment(\.dismiss) private var dismiss
    @State private var selectedPrintings: [String: Int] = [:] // entry ID -> selected printing index
    @State private var availablePrintings: [String: [CardPrintingData]] = [:] // card name -> printings
    @State private var isLoadingPrintings = false
    @State private var errorMessage: String?
    @State private var showCardList = false
    
    var body: some View {
        NavigationView {
            VStack(spacing: 16) {
                if entries.isEmpty {
                    Text("No cards found in decklist")
                        .foregroundColor(.secondary)
                        .font(.subheadline)
                } else {
                    // Header info
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Preview & Print Selection")
                            .font(.title2)
                            .fontWeight(.semibold)
                        
                        Text("\(entries.count) unique cards found")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal)
                    
                    // Background loading happens automatically - no progress indicator needed
                    
                    // 3x3 Grid Preview (matching desktop app)
                    VStack(spacing: 12) {
                        Text("Preview Grid")
                            .font(.headline)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal)
                        
                        GridPreviewSection(
                            entries: entries,
                            selectedPrintings: selectedPrintings,
                            availablePrintings: availablePrintings,
                            currentPage: 0 // TODO: Add page navigation
                        )
                        .padding(.horizontal)
                        
                        // Card Selection List (collapsible)
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
                                        ForEach(Array(entries.enumerated()), id: \.offset) { index, entry in
                                            CardEntryRow(
                                                entry: entry,
                                                selectedPrintingIndex: selectedPrintings[entryKey(for: entry)] ?? 0,
                                                availablePrintings: availablePrintings[entry.name] ?? [],
                                                onPrintingSelected: { printingIndex in
                                                    selectedPrintings[entryKey(for: entry)] = printingIndex
                                                }
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
    let entries: [DecklistEntryData]
    let selectedPrintings: [String: Int]
    let availablePrintings: [String: [CardPrintingData]]
    let currentPage: Int
    
    private let gridColumns = Array(repeating: GridItem(.flexible(), spacing: 4), count: 3)
    private let cardsPerPage = 9
    
    var body: some View {
        VStack(spacing: 8) {
            // Grid layout (3x3 matching desktop app)
            LazyVGrid(columns: gridColumns, spacing: 4) {
                ForEach(0..<cardsPerPage, id: \.self) { index in
                    GridCardView(
                        entry: getEntryForGridPosition(index),
                        selectedPrintings: selectedPrintings,
                        availablePrintings: availablePrintings
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
    
    private func getEntryForGridPosition(_ position: Int) -> DecklistEntryData? {
        // For now, just cycle through entries to fill the grid
        // TODO: Implement proper page logic that accounts for multiple copies
        let expandedEntries = entries.flatMap { entry in
            Array(repeating: entry, count: Int(entry.multiple))
        }
        
        let startIndex = currentPage * cardsPerPage
        let targetIndex = startIndex + position
        
        return targetIndex < expandedEntries.count ? expandedEntries[targetIndex] : nil
    }
}

struct GridCardView: View {
    let entry: DecklistEntryData?
    let selectedPrintings: [String: Int]
    let availablePrintings: [String: [CardPrintingData]]
    
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
            
            if let entry = entry {
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
                        
                        Text(entry.name)
                            .font(.caption2)
                            .multilineTextAlignment(.center)
                            .foregroundColor(.primary)
                            .lineLimit(3)
                            .padding(.horizontal, 4)
                        
                        if let set = entry.set {
                            Text("[\(set.uppercased())]")
                                .font(.caption2)
                                .foregroundColor(.blue)
                        }
                    }
                }
            } else {
                // Empty grid position
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color.clear)
            }
        }
        .onAppear {
            if let entry = entry {
                loadImageForEntry(entry)
            }
        }
        .onChange(of: selectedPrintings) { _ in
            if let entry = entry {
                loadImageForEntry(entry)
            }
        }
    }
    
    private func loadImageForEntry(_ entry: DecklistEntryData) {
        // Get the selected printing for this entry
        let entryKey = entryKey(for: entry)
        let selectedIndex = selectedPrintings[entryKey] ?? 0
        
        guard let printings = availablePrintings[entry.name],
              selectedIndex < printings.count else {
            imageData = nil
            return
        }
        
        let selectedPrinting = printings[selectedIndex]
        let imageURL = selectedPrinting.borderCropURL
        
        // Check if image is already cached
        if ProxyGenerator.isImageCached(for: imageURL) {
            // Load from cache
            switch ProxyGenerator.getCachedImageData(for: imageURL) {
            case .success(let data):
                imageData = data
                isLoadingImage = false
            case .failure(.imageNotCached):
                // Image was cached but now isn't - this shouldn't happen but handle gracefully
                imageData = nil
                isLoadingImage = false
            case .failure:
                // Other error - show placeholder
                imageData = nil
                isLoadingImage = false
            }
        } else {
            // Image not cached - show placeholder
            imageData = nil
            isLoadingImage = false
            // Note: We don't trigger network loading here since the user specified
            // that all images should come from the core library's cache
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
}

#Preview {
    PrintSelectionView(
        entries: [
            DecklistEntryData(multiple: 4, name: "Lightning Bolt", set: "lea", faceMode: .bothSides),
            DecklistEntryData(multiple: 1, name: "Counterspell", language: "ja", faceMode: .frontOnly)
        ],
        onGeneratePDF: {}
    )
}