use log::debug;
use ngrammatic::{Corpus, CorpusBuilder};
use ord_subset::OrdVar;
use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, PartialOrd, Ord)]
pub enum NameMatchMode {
    Full,
    Part(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub struct NameLookupResult {
    pub name: String,
    pub hit: NameMatchMode,
}

#[derive(Debug, Clone, PartialOrd, Ord, Eq, PartialEq)]
struct CorpusLookupResult {
    similarity: OrdVar<f32>,
    name: String,
}

#[derive(Debug)]
struct CardCorpus {
    corpus: Corpus,
    to_full: HashMap<String, String>,
}

impl CardCorpus {
    const THRESHOLD: f32 = 0.25;

    fn new() -> CardCorpus {
        CardCorpus {
            corpus: CorpusBuilder::new().finish(),
            to_full: HashMap::new(),
        }
    }

    pub fn insert(&mut self, partial_name: &str, full_name: &str) {
        self.corpus.add_text(partial_name);
        if partial_name != full_name {
            self.to_full
                .insert(partial_name.to_string(), full_name.to_string());
        }
    }

    pub fn find(&self, name: &str) -> Option<CorpusLookupResult> {
        let n = self
            .corpus
            .search(name, CardCorpus::THRESHOLD)
            .into_iter()
            .next()?;
        Some(CorpusLookupResult {
            name: self.to_full.get(n.text.as_str()).unwrap_or(&n.text).clone(),
            similarity: OrdVar::new_checked(n.similarity)?,
        })
    }
}

#[derive(Debug)]
pub struct CardNameLookup {
    corpora: HashMap<NameMatchMode, CardCorpus>,
}

impl CardNameLookup {
    fn new() -> CardNameLookup {
        CardNameLookup {
            corpora: HashMap::new(),
        }
    }

    pub fn from_card_names(names: &[String]) -> CardNameLookup {
        let mut lookup = CardNameLookup::new();
        for name in names.iter() {
            lookup.insert(name);
        }
        lookup
    }

    fn insert(&mut self, name_uppercase: &str) {
        let name = name_uppercase.to_lowercase();
        self.corpora
            .entry(NameMatchMode::Full)
            .or_insert_with(CardCorpus::new)
            .insert(&name, &name);

        if name.contains("//") {
            for (i, partial_name) in name.split("//").map(|s| s.trim()).enumerate() {
                self.corpora
                    .entry(NameMatchMode::Part(i))
                    .or_insert_with(CardCorpus::new)
                    .insert(partial_name, &name);
            }
        }
    }

    pub fn find(&self, name_uppercase: &str) -> Option<NameLookupResult> {
        let name = name_uppercase.to_lowercase();
        let best_match = self
            .corpora
            .iter()
            .filter_map(|(mode, c)| Some((c.find(&name)?, *mode)))
            .max_by(|(leftres, _), (rightres, _)| leftres.similarity.cmp(&rightres.similarity))?;
        debug!("similarity of best match: {:?}", best_match.0.similarity);
        Some(NameLookupResult {
            name: best_match.0.name.clone(),
            hit: best_match.1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_search() {
        let card_names: Vec<String> = vec![
            "Okaun, Eye of Chaos".to_string(),
            "Cut // Ribbons".to_string(),
        ];
        let lookup = CardNameLookup::from_card_names(&card_names);
        assert_eq!(
            lookup.find("okaun"),
            Some(NameLookupResult {
                name: "okaun, eye of chaos".to_string(),
                hit: NameMatchMode::Full
            })
        );
        assert_eq!(
            lookup.find("cut // ribbon"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Full
            })
        );
        assert_eq!(
            lookup.find("cut"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(0)
            })
        );
        assert_eq!(
            lookup.find("ribbon"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(1)
            })
        );
    }
}
