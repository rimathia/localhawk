use crate::DoubleFaceMode;
use crate::decklist::DecklistEntry;

/// Build aligned text output: start with original decklist, replace successfully parsed lines
/// Uses current parsed_cards state (which may have updated printings)
pub fn build_aligned_parsed_output(input_text: &str, parsed_cards: &[DecklistEntry]) -> String {
    let input_lines: Vec<&str> = input_text.lines().collect();
    let mut output_lines: Vec<String> = input_lines.iter().map(|line| line.to_string()).collect();

    // Replace lines where we successfully parsed something
    for entry in parsed_cards {
        if let Some(line_num) = entry.source_line_number {
            if line_num < output_lines.len() {
                let set_info = if let Some(set) = &entry.set {
                    format!(" • Set: {}", set.to_uppercase())
                } else {
                    String::new()
                };
                let lang_info = if let Some(lang) = &entry.lang {
                    format!(" • Lang: {}", lang.to_uppercase())
                } else {
                    String::new()
                };
                let face_info = match entry.face_mode {
                    DoubleFaceMode::FrontOnly => " • Face: Front only".to_string(),
                    DoubleFaceMode::BackOnly => " • Face: Back only".to_string(),
                    DoubleFaceMode::BothSides => " • Face: Both sides".to_string(),
                };

                output_lines[line_num] = format!(
                    "✓ {}x {}{}{}{}",
                    entry.multiple, entry.name, set_info, lang_info, face_info
                );
            }
        }
    }

    output_lines.join("\n")
}

/// Format a single decklist entry for display
pub fn format_decklist_entry(entry: &DecklistEntry) -> String {
    let set_info = if let Some(set) = &entry.set {
        format!(" • Set: {}", set.to_uppercase())
    } else {
        String::new()
    };
    let lang_info = if let Some(lang) = &entry.lang {
        format!(" • Lang: {}", lang.to_uppercase())
    } else {
        String::new()
    };
    let face_info = match entry.face_mode {
        DoubleFaceMode::FrontOnly => " • Face: Front only",
        DoubleFaceMode::BackOnly => " • Face: Back only",
        DoubleFaceMode::BothSides => " • Face: Both sides",
    };

    format!(
        "{}x {}{}{}{}",
        entry.multiple, entry.name, set_info, lang_info, face_info
    )
}

/// Format multiple entries as a summary
pub fn format_entries_summary(entries: &[DecklistEntry]) -> String {
    if entries.is_empty() {
        return "No cards".to_string();
    }

    let total_cards: u32 = entries.iter().map(|e| e.multiple as u32).sum();
    let unique_cards = entries.len();

    if unique_cards == 1 {
        format!("{} cards (1 unique)", total_cards)
    } else {
        format!("{} cards ({} unique)", total_cards, unique_cards)
    }
}
