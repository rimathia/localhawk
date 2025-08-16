import SwiftUI

struct ContentView: View {
    @State private var decklistText = """

1 Gisela, the Broken Blade
1 Bruna, the Fading Light
1 Counterspell [7ED]
// comments are ignored
1 Memory Lapse [ja]
1 kabira takedown
1 kabira plateau
1 cut // ribbons (pakh)
"""
    @State private var isGenerating = false
    @State private var pdfData: Data?
    @State private var errorMessage: String?
    @State private var showingShareSheet = false
    @State private var showingAdvancedOptions = false
    @State private var showingPrintSelection = false
    
    // Print selection state
    @State private var decklistEntries: [DecklistEntryData] = []
    @State private var globalFaceMode: DoubleFaceMode = .bothSides
    
    // Background loading is now fire-and-forget, no state tracking needed
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Decklist")
                        .font(.headline)
                        .foregroundColor(.primary)
                    
                    TextEditor(text: $decklistText)
                        .font(.system(.body, design: .monospaced))
                        .padding(8)
                        .background(Color(UIColor.secondarySystemBackground))
                        .cornerRadius(8)
                        .frame(minHeight: 200)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(Color(UIColor.separator), lineWidth: 1)
                        )
                }
                
                // Dual workflow buttons
                HStack(spacing: 12) {
                    // Simple workflow: Direct PDF generation  
                    Button(action: generatePDFDirectly) {
                        HStack {
                            if isGenerating {
                                ProgressView()
                                    .scaleEffect(0.8)
                                    .foregroundColor(.white)
                            } else {
                                Image(systemName: "doc.fill")
                            }
                            Text(isGenerating ? "Generating..." : "Generate PDF")
                        }
                        .foregroundColor(.white)
                        .padding()
                        .frame(maxWidth: .infinity)
                        .background(isGenerating ? Color.gray : Color.blue)
                        .cornerRadius(10)
                    }
                    .disabled(isGenerating || decklistText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    
                    // Advanced workflow: Preview first
                    Button(action: startPreviewWorkflow) {
                        HStack {
                            Image(systemName: "photo.on.rectangle.angled")
                            Text("Preview & Select")
                        }
                        .foregroundColor(.blue)
                        .padding()
                        .frame(maxWidth: .infinity)
                        .background(Color(UIColor.secondarySystemBackground))
                        .cornerRadius(10)
                        .overlay(
                            RoundedRectangle(cornerRadius: 10)
                                .stroke(Color.blue, lineWidth: 1)
                        )
                    }
                    .disabled(isGenerating || decklistText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
                
                if let errorMessage = errorMessage {
                    Text(errorMessage)
                        .foregroundColor(.red)
                        .font(.caption)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }
                
                if pdfData != nil {
                    Button(action: { showingShareSheet = true }) {
                        HStack {
                            Image(systemName: "square.and.arrow.up")
                            Text("Share PDF")
                        }
                        .foregroundColor(.blue)
                        .padding()
                        .frame(maxWidth: .infinity)
                        .background(Color(UIColor.secondarySystemBackground))
                        .cornerRadius(10)
                    }
                }
                
                Spacer()
                
                // Simple test of FFI connection
                VStack {
                    Text("FFI Test Result: \(ProxyGenerator.testConnection())")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .padding()
            .navigationTitle("LocalHawk")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        showingAdvancedOptions = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                }
            }
        }
        .navigationViewStyle(.stack)
        .sheet(isPresented: $showingShareSheet) {
            if let pdfData = pdfData {
                ShareSheet(items: [pdfData])
            }
        }
        .sheet(isPresented: $showingAdvancedOptions) {
            AdvancedOptionsView()
        }
        .sheet(isPresented: $showingPrintSelection) {
            PrintSelectionView(
                entries: decklistEntries,
                onGeneratePDF: generatePDFFromSelection
            )
        }
        .onDisappear {
            // Background loading is now fire-and-forget, no cleanup needed
        }
    }
    
    // MARK: - Workflow Functions
    
    /// Direct PDF generation workflow (existing behavior)
    private func generatePDFDirectly() {
        guard !decklistText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            errorMessage = "Please enter a decklist"
            return
        }
        
        isGenerating = true
        errorMessage = nil
        pdfData = nil
        
        // Use Task with appropriate priority for PDF generation
        // .utility is good for user-initiated but heavy work like PDF generation
        Task(priority: .utility) {
            let result = ProxyGenerator.generatePDF(from: decklistText.trimmingCharacters(in: .whitespacesAndNewlines))
            
            await MainActor.run {
                isGenerating = false
                
                switch result {
                case .success(let data):
                    pdfData = data
                    errorMessage = nil
                case .failure(let error):
                    pdfData = nil
                    errorMessage = "Failed to generate PDF: \(error.localizedDescription)"
                }
            }
        }
    }
    
    /// Preview-first workflow with print selection
    private func startPreviewWorkflow() {
        guard !decklistText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            errorMessage = "Please enter a decklist"
            return
        }
        
        isGenerating = true
        errorMessage = nil
        
        // Use the new combined parse + background loading function
        Task(priority: .utility) {
            let result = ProxyGenerator.parseAndStartBackgroundLoading(
                decklistText.trimmingCharacters(in: .whitespacesAndNewlines),
                globalFaceMode: globalFaceMode
            )
            
            await MainActor.run {
                isGenerating = false
                
                switch result {
                case .success(let entries):
                    decklistEntries = entries
                    errorMessage = nil
                    showingPrintSelection = true
                    // Background loading is now started automatically by the core library
                case .failure(let error):
                    decklistEntries = []
                    errorMessage = "Failed to parse decklist: \(error.localizedDescription)"
                }
            }
        }
    }
    
    /// Generate PDF from print selection (called from PrintSelectionView)
    private func generatePDFFromSelection() {
        // For now, fall back to direct PDF generation
        // TODO: Implement PDF generation from selected printings
        generatePDFDirectly()
        showingPrintSelection = false
    }
    
    // MARK: - Background loading is now handled automatically by the core library
}

// Helper struct for sharing PDFs
struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]
    
    func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(activityItems: items, applicationActivities: nil)
        return controller
    }
    
    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {
        // No updates needed
    }
}

#Preview {
    ContentView()
}
