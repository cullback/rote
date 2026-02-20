use chrono::NaiveDate;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Card {
    pub deck: String,
    pub front: String,
    pub back: String,
    pub media: String,
    pub id: String,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub due: Option<NaiveDate>,
    pub last_review: Option<NaiveDate>,
}

pub fn extract_cloze_deletions(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in text.chars() {
        match ch {
            '[' => {
                if depth == 0 {
                    current.clear();
                } else {
                    current.push(ch);
                }
                depth += 1;
            }
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && !current.is_empty() {
                    results.push(current.clone());
                    current.clear();
                } else if depth > 0 {
                    current.push(ch);
                }
            }
            _ => {
                if depth > 0 {
                    current.push(ch);
                }
            }
        }
    }
    results
}

pub fn expand_newlines(s: &str) -> String {
    s.replace("\\n", "\n")
}

fn parse_optional_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() { None } else { s.parse().ok() }
}

fn parse_optional_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
    }
}

fn get_field(record: &csv::StringRecord, index: usize) -> String {
    record.get(index).unwrap_or("").to_string()
}

pub fn load_csv(path: &Path) -> Result<Vec<Card>, String> {
    let default_deck = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_string();

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("failed to open {}: {}", path.display(), e))?;

    let mut cards = Vec::new();
    for result in reader.records() {
        let record = result.map_err(|e| format!("CSV parse error in {}: {}", path.display(), e))?;

        let deck_raw = get_field(&record, 0);
        let deck = if deck_raw.trim().is_empty() {
            default_deck.clone()
        } else {
            deck_raw
        };

        let id_raw = get_field(&record, 4);
        let id = if id_raw.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            id_raw
        };

        cards.push(Card {
            deck,
            front: get_field(&record, 1),
            back: get_field(&record, 2),
            media: get_field(&record, 3),
            id,
            stability: parse_optional_f64(&get_field(&record, 5)),
            difficulty: parse_optional_f64(&get_field(&record, 6)),
            due: parse_optional_date(&get_field(&record, 7)),
            last_review: parse_optional_date(&get_field(&record, 8)),
        });
    }
    Ok(cards)
}

pub fn save_csv(path: &Path, cards: &[Card]) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path)
        .map_err(|e| format!("failed to write {}: {}", path.display(), e))?;

    writer
        .write_record([
            "deck",
            "front",
            "back",
            "media",
            "id",
            "stability",
            "difficulty",
            "due",
            "last_review",
        ])
        .map_err(|e| format!("write error: {e}"))?;

    for card in cards {
        writer
            .write_record([
                &card.deck,
                &card.front,
                &card.back,
                &card.media,
                &card.id,
                &card.stability.map_or(String::new(), |v| format!("{v:.3}")),
                &card.difficulty.map_or(String::new(), |v| format!("{v:.3}")),
                &card
                    .due
                    .map_or(String::new(), |d| d.format("%Y-%m-%d").to_string()),
                &card
                    .last_review
                    .map_or(String::new(), |d| d.format("%Y-%m-%d").to_string()),
            ])
            .map_err(|e| format!("write error: {e}"))?;
    }

    writer.flush().map_err(|e| format!("flush error: {e}"))?;
    Ok(())
}

pub fn discover_files(paths: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for p in paths {
        let path = PathBuf::from(p);
        if path.is_dir() {
            collect_csv_recursive(&path, &mut files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("csv") {
            files.push(path);
        }
    }
    files
}

fn collect_csv_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_csv_recursive(&path, files);
        } else if path.extension().and_then(|e| e.to_str()) == Some("csv") {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn cloze_extraction() {
        assert_eq!(
            extract_cloze_deletions("The [mitochondria] is the [powerhouse] of the cell"),
            vec!["mitochondria", "powerhouse"]
        );
    }

    #[test]
    fn cloze_extraction_empty() {
        assert!(extract_cloze_deletions("No brackets here").is_empty());
    }

    #[test]
    fn cloze_extraction_nested() {
        assert_eq!(
            extract_cloze_deletions("A [nested [bracket]] test"),
            vec!["nested [bracket]"]
        );
    }

    #[test]
    fn expand_newlines_works() {
        assert_eq!(expand_newlines("line1\\nline2"), "line1\nline2");
        assert_eq!(expand_newlines("no newlines"), "no newlines");
    }

    #[test]
    fn csv_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.csv");

        let cards = vec![Card {
            deck: "math".to_string(),
            front: "What is 2+2?".to_string(),
            back: "4".to_string(),
            media: String::new(),
            id: "test-id-1".to_string(),
            stability: Some(3.173),
            difficulty: Some(5.5),
            due: NaiveDate::from_ymd_opt(2025, 6, 15),
            last_review: NaiveDate::from_ymd_opt(2025, 6, 1),
        }];

        save_csv(&path, &cards).unwrap();
        let loaded = load_csv(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].deck, "math");
        assert_eq!(loaded[0].front, "What is 2+2?");
        assert_eq!(loaded[0].back, "4");
        assert_eq!(loaded[0].id, "test-id-1");
        assert!((loaded[0].stability.unwrap() - 3.173).abs() < 0.01);
        assert!((loaded[0].difficulty.unwrap() - 5.5).abs() < 0.01);
        assert_eq!(loaded[0].due, NaiveDate::from_ymd_opt(2025, 6, 15));
        assert_eq!(loaded[0].last_review, NaiveDate::from_ymd_opt(2025, 6, 1));
    }

    #[test]
    fn csv_missing_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sparse.csv");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(
                f,
                "deck,front,back,media,id,stability,difficulty,due,last_review"
            )
            .unwrap();
            writeln!(f, ",What is Rust?,A language,,,,,").unwrap();
        }
        let cards = load_csv(&path).unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].deck, "sparse");
        assert_eq!(cards[0].front, "What is Rust?");
        assert!(!cards[0].id.is_empty());
        assert!(cards[0].stability.is_none());
    }

    #[test]
    fn discover_files_works() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(dir.path().join("a.csv"), "").unwrap();
        std::fs::write(sub.join("b.csv"), "").unwrap();
        std::fs::write(dir.path().join("c.txt"), "").unwrap();

        let files = discover_files(&[dir.path().to_str().unwrap().to_string()]);
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "csv"));
    }
}
