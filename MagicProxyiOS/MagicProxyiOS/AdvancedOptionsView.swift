import SwiftUI

struct AdvancedOptionsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var imageCacheStats = CacheStatistics(count: 0, sizeMB: 0.0)
    @State private var searchCacheStats = CacheStatistics(count: 0, sizeMB: 0.0)
    @State private var cardNamesCacheStats = CacheStatistics(count: 0, sizeMB: 0.0)
    @State private var isUpdatingCardNames = false
    @State private var errorMessage: String?
    @State private var successMessage: String?
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 24) {
                    // Cache Statistics Section
                    VStack(alignment: .leading, spacing: 16) {
                        Text("Cache Statistics")
                            .font(.title2)
                            .fontWeight(.semibold)
                            .padding(.bottom, 4)
                        
                        // Image Cache
                        CacheStatCard(
                            title: "Image Cache",
                            subtitle: "Downloaded card images",
                            stats: [
                                ("Images cached", "\(imageCacheStats.count) items"),
                                ("Cache size estimate", String(format: "%.1f MB", imageCacheStats.sizeMB)),
                                ("Max size", "1000 MB")
                            ],
                            locationPath: ProxyGenerator.getImageCachePath() ?? "Unknown",
                            color: .blue,
                            actionTitle: "Clear",
                            action: clearImageCache
                        )
                        
                        // Search Results Cache
                        CacheStatCard(
                            title: "Search Results Cache",
                            subtitle: "Cached card searches",
                            stats: [
                                ("Searches cached", "\(searchCacheStats.count) items"),
                                ("Cache size estimate", String(format: "%.1f MB", searchCacheStats.sizeMB)),
                                ("Max entries", "1000 searches")
                            ],
                            locationPath: ProxyGenerator.getSearchCachePath() ?? "Unknown",
                            color: .orange,
                            actionTitle: "Clear",
                            action: { /* Search cache clear not implemented yet */ }
                        )
                        
                        // Card Names Database
                        CacheStatCard(
                            title: "Card Names Database",
                            subtitle: "Fuzzy search index",
                            stats: [
                                ("Card names", "\(cardNamesCacheStats.count) items"),
                                ("Database size estimate", String(format: "%.1f MB", cardNamesCacheStats.sizeMB)),
                                ("Status", cardNamesCacheStats.count > 0 ? "Loaded" : "Not loaded")
                            ],
                            locationPath: ProxyGenerator.getCardNamesCachePath() ?? "Unknown",
                            color: .green,
                            actionTitle: isUpdatingCardNames ? "Updating..." : "Update",
                            action: updateCardNames,
                            isActionDisabled: isUpdatingCardNames
                        )
                    }
                    
                    // Status Messages
                    if let errorMessage = errorMessage {
                        Text(errorMessage)
                            .foregroundColor(.red)
                            .font(.caption)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }
                    
                    if let successMessage = successMessage {
                        Text(successMessage)
                            .foregroundColor(.green)
                            .font(.caption)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }
                }
                .padding()
            }
            .navigationTitle("Advanced Options")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
        .onAppear {
            loadCacheStatistics()
        }
    }
    
    private func loadCacheStatistics() {
        imageCacheStats = ProxyGenerator.getImageCacheStats()
        searchCacheStats = ProxyGenerator.getSearchCacheStats()
        cardNamesCacheStats = ProxyGenerator.getCardNamesCacheStats()
    }
    
    private func clearImageCache() {
        switch ProxyGenerator.clearImageCache() {
        case .success:
            successMessage = "Image cache cleared successfully"
            errorMessage = nil
            loadCacheStatistics()
            
            // Clear success message after 3 seconds
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                successMessage = nil
            }
            
        case .failure(let error):
            errorMessage = "Failed to clear image cache: \(error.localizedDescription)"
            successMessage = nil
        }
    }
    
    private func updateCardNames() {
        isUpdatingCardNames = true
        errorMessage = nil
        successMessage = nil
        
        // Run on background queue since this is a network operation
        DispatchQueue.global(qos: .userInitiated).async {
            let result = ProxyGenerator.updateCardNames()
            
            DispatchQueue.main.async {
                isUpdatingCardNames = false
                
                switch result {
                case .success:
                    successMessage = "Card names database updated successfully"
                    errorMessage = nil
                    loadCacheStatistics()
                    
                    // Clear success message after 3 seconds
                    DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                        successMessage = nil
                    }
                    
                case .failure(let error):
                    errorMessage = "Failed to update card names: \(error.localizedDescription)"
                    successMessage = nil
                }
            }
        }
    }
    
}

struct CacheStatCard: View {
    let title: String
    let subtitle: String
    let stats: [(String, String)]
    let locationPath: String
    let color: Color
    let actionTitle: String
    let action: () -> Void
    var isActionDisabled: Bool = false
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.subheadline)
                        .fontWeight(.semibold)
                    Text(subtitle)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                Spacer()
                Button(actionTitle, action: action)
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(isActionDisabled)
            }
            
            VStack(alignment: .leading, spacing: 4) {
                ForEach(stats, id: \.0) { stat in
                    HStack {
                        Text("• \(stat.0)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        Spacer()
                        Text(stat.1)
                            .font(.caption)
                            .fontWeight(.medium)
                    }
                }
                
                // Location path with expandable display
                HStack(alignment: .top) {
                    Text("• Location")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    Spacer()
                    ExpandablePathView(path: locationPath)
                }
            }
        }
        .padding(16)
        .background(color.opacity(0.1))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(color.opacity(0.3), lineWidth: 1)
        )
    }
}

struct ExpandablePathView: View {
    let path: String
    @State private var isExpanded = false
    
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(isExpanded ? path : truncatedPath)
                .font(.caption)
                .fontWeight(.medium)
                .multilineTextAlignment(.leading)
                .lineLimit(isExpanded ? nil : 1)
                .onTapGesture {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        isExpanded.toggle()
                    }
                }
        }
    }
    
    private var truncatedPath: String {
        // Try to fit as much of the tail as possible on one line
        // This is a simplified approach - in production you might want to measure actual text width
        let maxLength = 35 // Approximate character limit for one line
        
        if path.count <= maxLength {
            return path
        }
        
        // Show "..." at the beginning followed by the tail
        let tailLength = maxLength - 3 // Account for "..."
        let startIndex = path.index(path.endIndex, offsetBy: -tailLength)
        return "..." + path[startIndex...]
    }
}

#Preview {
    AdvancedOptionsView()
}
