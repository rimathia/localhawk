use directories::ProjectDirs;
use std::fs;

fn main() {
    println!("Testing cache directory locations for localhawk...\n");
    
    if let Some(proj_dirs) = ProjectDirs::from("", "", "localhawk") {
        println!("Cache directory: {}", proj_dirs.cache_dir().display());
        println!("Config directory: {}", proj_dirs.config_dir().display());
        println!("Data directory: {}", proj_dirs.data_dir().display());
        
        let cache_dir = proj_dirs.cache_dir();
        let cache_file = cache_dir.join("card_names.json");
        
        println!("\nFull cache file path: {}", cache_file.display());
        
        // Check if cache directory exists
        if cache_dir.exists() {
            println!("✅ Cache directory exists");
            
            // Check if cache file exists
            if cache_file.exists() {
                println!("✅ Cache file exists");
                
                // Show file size
                if let Ok(metadata) = fs::metadata(&cache_file) {
                    println!("📁 Cache file size: {} bytes", metadata.len());
                }
                
                // Show last modified time
                if let Ok(metadata) = fs::metadata(&cache_file) {
                    if let Ok(modified) = metadata.modified() {
                        println!("🕒 Last modified: {:?}", modified);
                    }
                }
            } else {
                println!("❌ Cache file does not exist yet");
            }
        } else {
            println!("❌ Cache directory does not exist yet");
        }
        
        println!("\nTo create the cache, run the localhawk-gui and use fuzzy matching.");
    } else {
        println!("❌ Could not determine cache directories for this platform");
    }
}