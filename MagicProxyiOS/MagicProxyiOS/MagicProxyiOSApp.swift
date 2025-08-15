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
        }
    }
}