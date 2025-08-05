mod app;

fn init_logging() {
    // Initialize tracing with configurable filtering
    tracing_subscriber::fmt()
        .with_env_filter(
            // Default to info level, but allow override via RUST_LOG
            // Example: RUST_LOG=magic_proxy_core::globals=debug,magic_proxy_core::card_name_cache=debug
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "magic_proxy_core=info,magic_proxy_gui=info".into()),
        )
        .init();
}

fn main() -> iced::Result {
    init_logging();
    
    // Initialize caches at startup
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    if let Err(e) = rt.block_on(magic_proxy_core::initialize_caches()) {
        eprintln!("Failed to initialize caches: {}", e);
        std::process::exit(1);
    }
    
    // Run the GUI application
    let result = iced::application("Magic Card Proxy Generator", app::update, app::view)
        .run_with(app::initialize);
    
    // Application has exited (user closed window), save caches before returning
    if let Err(e) = rt.block_on(magic_proxy_core::shutdown_caches()) {
        eprintln!("Warning: Failed to save caches on shutdown: {}", e);
    }
    
    result
}