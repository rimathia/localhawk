use clap::{Parser, Subcommand};
use magic_proxy_core::{ProxyGenerator, PdfOptions};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "magic-proxy-cli")]
#[command(about = "A CLI for generating Magic: The Gathering proxy sheets")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for Magic cards
    Search {
        /// Card name to search for
        name: String,
    },
    /// Generate a PDF from a list of card names
    Generate {
        /// Card names (one per line or comma-separated)
        #[arg(short, long)]
        cards: Vec<String>,
        /// Output PDF file path
        #[arg(short, long, default_value = "proxies.pdf")]
        output: PathBuf,
        /// Number of cards per row (default: 3)
        #[arg(long, default_value = "3")]
        cards_per_row: u32,
        /// Number of cards per column (default: 3) 
        #[arg(long, default_value = "3")]
        cards_per_column: u32,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    let cli = Cli::parse();
    let mut generator = ProxyGenerator::new()?;

    match cli.command {
        Commands::Search { name } => {
            println!("Searching for '{}'...", name);
            
            match generator.search_card(&name).await {
                Ok(results) => {
                    println!("Found {} cards:", results.total_found);
                    for (i, card) in results.cards.iter().enumerate().take(10) {
                        println!("  {}. {} ({}) - {}", i + 1, card.name, card.set, card.language);
                    }
                    if results.cards.len() > 10 {
                        println!("  ... and {} more", results.cards.len() - 10);
                    }
                }
                Err(e) => {
                    eprintln!("Search failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Generate { cards, output, cards_per_row, cards_per_column } => {
            if cards.is_empty() {
                eprintln!("No cards specified. Use --cards to specify card names.");
                std::process::exit(1);
            }

            println!("Generating PDF with {} cards...", cards.len());
            
            // Search and add each card
            for card_name in cards {
                println!("Searching for '{}'...", card_name);
                match generator.search_card(&card_name).await {
                    Ok(results) => {
                        if let Some(card) = results.cards.first() {
                            generator.add_card(card.clone(), 1);
                            println!("  Added: {} ({})", card.name, card.set);
                        } else {
                            eprintln!("  No results found for '{}'", card_name);
                        }
                    }
                    Err(e) => {
                        eprintln!("  Search failed for '{}': {}", card_name, e);
                    }
                }
            }

            if generator.get_cards().is_empty() {
                eprintln!("No valid cards found. Cannot generate PDF.");
                std::process::exit(1);
            }

            // Generate PDF
            let options = PdfOptions {
                cards_per_row,
                cards_per_column,
                ..Default::default()
            };

            println!("Generating PDF...");
            match generator.generate_pdf(options, |current, total| {
                println!("Progress: {}/{}", current, total);
            }).await {
                Ok(pdf_data) => {
                    std::fs::write(&output, pdf_data)?;
                    println!("PDF saved to: {}", output.display());
                    println!("Cache size: {} images", generator.cache_size());
                }
                Err(e) => {
                    eprintln!("PDF generation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}