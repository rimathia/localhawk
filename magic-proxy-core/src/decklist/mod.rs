use regex::{Match, Regex};
use std::collections::HashSet;
use lazy_static::lazy_static;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DecklistEntry {
    pub multiple: i32,
    pub name: String,
    pub set: Option<String>,
    pub lang: Option<String>,
    pub preferred_face: Option<crate::lookup::NameMatchMode>, // Track which face the user wanted
}

impl DecklistEntry {
    pub fn new(multiple: i32, name: &str, set: Option<&str>, lang: Option<&str>) -> DecklistEntry {
        DecklistEntry {
            multiple,
            name: name.to_string(),
            set: set.map(String::from),
            lang: lang.map(String::from),
            preferred_face: None,
        }
    }

    pub fn from_name(n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: 1,
            name: n.to_string(),
            set: None,
            lang: None,
            preferred_face: None,
        }
    }

    pub fn from_multiple_name(m: i32, n: &str) -> DecklistEntry {
        DecklistEntry {
            multiple: m,
            name: n.to_string(),
            set: None,
            lang: None,
            preferred_face: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParsedDecklistLine<'a> {
    line: &'a str,
    entry: Option<DecklistEntry>,
}

impl ParsedDecklistLine<'_> {
    pub fn as_entry(&self) -> Option<DecklistEntry> {
        self.entry.clone()
    }
}

fn parse_multiple(group: Option<Match>) -> i32 {
    match group {
        Some(m) => m.as_str().parse().ok().unwrap_or(1),
        None => 1,
    }
}


fn parse_set_and_lang(group: Option<Match>, languages: &HashSet<String>, set_codes: &HashSet<String>) -> (Option<String>, Option<String>) {
    if let Some(code) = group {
        let code_str = code.as_str().to_lowercase();
        
        if set_codes.contains(&code_str) {
            // It's a valid set code
            (Some(code_str), None)
        } else if languages.contains(&code_str) {
            // It's a language code
            (None, Some(code_str))
        } else {
            // Unknown code - default to treating as set (for backward compatibility)
            (Some(code_str), None)
        }
    } else {
        (None, None)
    }
}

pub fn parse_line(line: &str, languages: &HashSet<String>, set_codes: &HashSet<String>) -> Option<DecklistEntry> {
    lazy_static! {
        static ref REMNS: Regex =
            Regex::new(r"^\s*(\d*)\s*([^\(\[\$\t]*)[\s\(\[]*([\dA-Za-z]{2,6})?").unwrap();
    }

    match REMNS.captures(line) {
        Some(mns) => {
            let multiple = parse_multiple(mns.get(1));
            let name = mns.get(2)?.as_str().trim().to_string();
            let set_or_lang = mns.get(3);
            let (set, lang) = parse_set_and_lang(set_or_lang, languages, set_codes);
            log::debug!("Parsed decklist line '{}' -> name: '{}', set: {:?}, lang: {:?}", line.trim(), name, set, lang);
            let name_lowercase = name.to_lowercase();
            let non_entries = ["deck", "decklist", "sideboard"];
            if non_entries.iter().any(|s| **s == name_lowercase) {
                None
            } else {
                Some(DecklistEntry {
                    multiple,
                    name,
                    set,
                    lang,
                    preferred_face: None, // Will be set during name resolution
                })
            }
        }
        None => None,
    }
}

pub fn parse_decklist<'a>(
    decklist: &'a str,
    languages: &HashSet<String>,
    set_codes: &HashSet<String>,
) -> Vec<ParsedDecklistLine<'a>> {
    decklist
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| ParsedDecklistLine {
            line: s,
            entry: parse_line(s, languages, set_codes),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::models::get_minimal_scryfall_languages;

    fn parse_line_default(s: &str) -> Option<DecklistEntry> {
        let minimal = get_minimal_scryfall_languages();
        let set_codes = std::collections::HashSet::new(); // Empty for tests
        parse_line(s, &minimal, &set_codes)
    }

    fn parse_decklist_default(s: &str) -> Vec<ParsedDecklistLine> {
        let minimal = get_minimal_scryfall_languages();
        let set_codes = std::collections::HashSet::new(); // Empty for tests
        parse_decklist(s, &minimal, &set_codes)
    }

    #[test]
    fn name() {
        assert_eq!(
            parse_line_default("plains").unwrap(),
            DecklistEntry::from_name("plains")
        );
    }

    #[test]
    fn number_name() {
        assert_eq!(
            parse_line_default("2\tplains").unwrap(),
            DecklistEntry::from_multiple_name(2, "plains")
        );
    }

    #[test]
    fn shatter() {
        assert_eq!(
            parse_line_default("1 shatter [mrd]").unwrap(),
            DecklistEntry::new(1, "shatter", Some("mrd"), None)
        );
    }

    #[test]
    fn number_name_set() {
        assert_eq!(
            parse_line_default("17 long card's name [IPA]").unwrap(),
            DecklistEntry::new(17, "long card's name", Some("ipa"), None)
        );
    }

    #[test]
    fn name_set() {
        assert_eq!(
            parse_line_default("long card's name [ipa]").unwrap(),
            DecklistEntry::new(1, "long card's name", Some("ipa"), None)
        );
    }

    #[test]
    fn name_with_tab() {
        assert_eq!(
            parse_line_default("Incubation/Incongruity   \t\t---").unwrap(),
            DecklistEntry::from_multiple_name(1, "Incubation/Incongruity")
        );
    }

    #[test]
    fn japanese_printing() {
        assert_eq!(
            parse_line_default("memory lapse [ja]").unwrap(),
            DecklistEntry::new(1, "memory lapse", None, Some("ja")) // Only lang="ja", not set="ja"
        );
    }

    #[test]
    fn mtgdecks() {
        let decklist = "4  Beanstalk Giant   \t\t$0.25
        4  Lovestruck Beast   \t\t$1.5
        Artifact [5]
        1  The Great Henge   \t\t$25
        Instant [1]
        1  Incubation/Incongruity   \t\t--- ";
        let parsed = parse_decklist_default(decklist);
        let expected = vec![
            ParsedDecklistLine {
                line: "4  Beanstalk Giant   \t\t$0.25",
                entry: Some(DecklistEntry::from_multiple_name(4, "Beanstalk Giant")),
            },
            ParsedDecklistLine {
                line: "4  Lovestruck Beast   \t\t$1.5",
                entry: Some(DecklistEntry::from_multiple_name(4, "Lovestruck Beast")),
            },
            ParsedDecklistLine {
                line: "Artifact [5]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Artifact")),
            },
            ParsedDecklistLine {
                line: "1  The Great Henge   \t\t$25",
                entry: Some(DecklistEntry::from_multiple_name(1, "The Great Henge")),
            },
            ParsedDecklistLine {
                line: "Instant [1]",
                entry: Some(DecklistEntry::from_multiple_name(1, "Instant")),
            },
            ParsedDecklistLine {
                line: "1  Incubation/Incongruity   \t\t---",
                entry: Some(DecklistEntry::from_multiple_name(
                    1,
                    "Incubation/Incongruity",
                )),
            },
        ];
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn arenaexport() {
        let decklist = "Deck
        1 Bedeck // Bedazzle (RNA) 221
        1 Spawn of Mayhem (RNA) 85
        ";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: None,
            },
            ParsedDecklistLine {
                line: "1 Bedeck // Bedazzle (RNA) 221",
                entry: Some(DecklistEntry::new(
                    1,
                    "Bedeck // Bedazzle",
                    Some("rna"),
                    None,
                )),
            },
            ParsedDecklistLine {
                line: "1 Spawn of Mayhem (RNA) 85",
                entry: Some(DecklistEntry::new(1, "Spawn of Mayhem", Some("rna"), None)),
            },
        ];
        let parsed = parse_decklist_default(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn arenaexport2() {
        let decklist = "Deck\n1 Defiant Strike (M21) 15\n24 Plains (ANB) 115\n\nSideboard\n2 Faerie Guidemother (ELD) 11";
        let expected = vec![
            ParsedDecklistLine {
                line: "Deck",
                entry: None,
            },
            ParsedDecklistLine {
                line: "1 Defiant Strike (M21) 15",
                entry: Some(DecklistEntry::new(1, "Defiant Strike", Some("m21"), None)),
            },
            ParsedDecklistLine {
                line: "24 Plains (ANB) 115",
                entry: Some(DecklistEntry::new(24, "Plains", Some("anb"), None)),
            },
            ParsedDecklistLine {
                line: "Sideboard",
                entry: None,
            },
            ParsedDecklistLine {
                line: "2 Faerie Guidemother (ELD) 11",
                entry: Some(DecklistEntry::new(
                    2,
                    "Faerie Guidemother",
                    Some("eld"),
                    None,
                )),
            },
        ];
        let parsed = parse_decklist_default(decklist);
        for (left, right) in parsed.iter().zip(expected.iter()) {
            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_various_set_codes_and_languages() {
        // Test with actual set codes from cache and various languages
        let mut set_codes = std::collections::HashSet::new();
        set_codes.insert("bro".to_string());     // 3 chars - standard
        set_codes.insert("plst".to_string());    // 4 chars - special product
        set_codes.insert("pakh".to_string());    // 4 chars - promo
        set_codes.insert("h2r".to_string());     // 3 chars with number
        set_codes.insert("pmps08".to_string());  // 6 chars - long promo code
        set_codes.insert("30a".to_string());     // 3 chars starting with number
        
        let languages = get_minimal_scryfall_languages();
        
        let test_cases = vec![
            ("1 Lightning Bolt [BRO]", Some(DecklistEntry::new(1, "Lightning Bolt", Some("bro"), None))),
            ("2 Cut // Ribbons [PLST]", Some(DecklistEntry::new(2, "Cut // Ribbons", Some("plst"), None))),
            ("3 Kabira Takedown [PAKH]", Some(DecklistEntry::new(3, "Kabira Takedown", Some("pakh"), None))),
            ("4 Memory Lapse [JA]", Some(DecklistEntry::new(4, "Memory Lapse", None, Some("ja")))),
            ("1 Brainstorm [FR]", Some(DecklistEntry::new(1, "Brainstorm", None, Some("fr")))),
            ("2 Giant Growth [DE]", Some(DecklistEntry::new(2, "Giant Growth", None, Some("de")))),
            ("1 Black Lotus [H2R]", Some(DecklistEntry::new(1, "Black Lotus", Some("h2r"), None))),
            ("3 Ancestral Recall [PMPS08]", Some(DecklistEntry::new(3, "Ancestral Recall", Some("pmps08"), None))),
            ("1 Time Walk [30A]", Some(DecklistEntry::new(1, "Time Walk", Some("30a"), None))),
            ("5 Counterspell", Some(DecklistEntry::new(5, "Counterspell", None, None))), // No set/lang
        ];
        
        for (input, expected) in test_cases {
            let result = parse_line(input, &languages, &set_codes);
            match (&result, &expected) {
                (Some(parsed), Some(exp)) => {
                    assert_eq!(parsed.multiple, exp.multiple, "Multiple mismatch for: {}", input);
                    assert_eq!(parsed.name, exp.name, "Name mismatch for: {}", input);
                    assert_eq!(parsed.set, exp.set, "Set mismatch for: {}", input);
                    assert_eq!(parsed.lang, exp.lang, "Language mismatch for: {}", input);
                    assert_eq!(parsed.preferred_face, None, "Preferred face should be None for: {}", input);
                },
                (None, None) => {}, // Both None, test passes
                _ => panic!("Mismatch for input '{}': got {:?}, expected {:?}", input, result, expected),
            }
        }
    }
}