import SwiftUI

struct ContentView: View {
    @State private var decklistText = """
1 Lightning Bolt
1 Counterspell
1 Giant Growth
1 Dark Ritual
"""
    @State private var isGenerating = false
    @State private var pdfData: Data?
    @State private var errorMessage: String?
    @State private var showingShareSheet = false
    @State private var showingAdvancedOptions = false
    
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
                
                Button(action: generatePDF) {
                    HStack {
                        if isGenerating {
                            ProgressView()
                                .scaleEffect(0.8)
                                .foregroundColor(.white)
                        } else {
                            Image(systemName: "doc.fill")
                        }
                        Text(isGenerating ? "Generating PDF..." : "Generate PDF")
                    }
                    .foregroundColor(.white)
                    .padding()
                    .frame(maxWidth: .infinity)
                    .background(isGenerating ? Color.gray : Color.blue)
                    .cornerRadius(10)
                }
                .disabled(isGenerating || decklistText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                
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
        .sheet(isPresented: $showingShareSheet) {
            if let pdfData = pdfData {
                ShareSheet(items: [pdfData])
            }
        }
        .sheet(isPresented: $showingAdvancedOptions) {
            // TODO: Add AdvancedOptionsView.swift file to Xcode project
            Text("Advanced Options")
            AdvancedOptionsView()
        }
    }
    
    private func generatePDF() {
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
