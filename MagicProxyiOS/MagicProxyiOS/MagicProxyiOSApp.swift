import SwiftUI

@main
struct MagicProxyiOSApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
                .onAppear {
                    // Initialize the Rust FFI layer when app starts
                    ProxyGenerator.initialize()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification)) { _ in
                    // Save caches when app goes to background
                    saveCachesInBackground()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willTerminateNotification)) { _ in
                    // Save caches when app terminates
                    saveCachesInBackground()
                }
        }
    }
    
    private func saveCachesInBackground() {
        DispatchQueue.global(qos: .utility).async {
            switch ProxyGenerator.saveCaches() {
            case .success:
                print("✅ Caches saved successfully")
            case .failure(let error):
                print("❌ Failed to save caches: \(error.localizedDescription)")
            }
        }
    }
}