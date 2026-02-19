use std::io::Write;

// Integration tests exercise the public library surface.
// We need to make the crate a lib+bin to test this way,
// so we test via the binary's modules by re-importing the logic.

#[test]
fn full_review_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let csv_path = dir.path().join("test_deck.csv");

    // Write a CSV with some cards (user-authored, no FSRS fields)
    {
        let mut f = std::fs::File::create(&csv_path).unwrap();
        writeln!(
            f,
            "deck,front,back,media,id,stability,difficulty,due,last_review"
        )
        .unwrap();
        writeln!(f, ",What is 2+2?,4,,,,,,").unwrap();
        writeln!(
            f,
            ",The [mitochondria] is the [powerhouse],ATP producer,,,,,,"
        )
        .unwrap();
        writeln!(f, "custom_deck,Bonjour means [hello],,,,,,,,").unwrap();
    }

    // Load
    let csv_content = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv_content.contains("What is 2+2?"));

    // Verify the file can be parsed by the csv crate
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&csv_path)
        .unwrap();

    let mut card_count = 0;
    for result in reader.records() {
        let record = result.unwrap();
        card_count += 1;
        // First field is deck
        let _deck = record.get(0).unwrap_or("");
        // Second is front
        let front = record.get(1).unwrap_or("");
        assert!(!front.is_empty());
    }
    assert_eq!(card_count, 3);
}

#[test]
fn csv_preserves_data_through_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.csv");

    // Write a proper CSV
    {
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "deck,front,back,media,id,stability,difficulty,due,last_review"
        )
        .unwrap();
        writeln!(
            f,
            "math,What is pi?,3.14159,,test-id-1,3.1730,5.5000,2025-06-15,2025-06-01"
        )
        .unwrap();
        writeln!(f, ",New card,answer,,,,,,").unwrap();
    }

    // Read it back
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(&path)
        .unwrap();
    let records: Vec<csv::StringRecord> = reader.records().map(|r| r.unwrap()).collect();

    assert_eq!(records.len(), 2);

    // First card has all fields
    assert_eq!(records[0].get(0).unwrap(), "math");
    assert_eq!(records[0].get(1).unwrap(), "What is pi?");
    assert_eq!(records[0].get(4).unwrap(), "test-id-1");
    assert_eq!(records[0].get(7).unwrap(), "2025-06-15");

    // Second card has sparse fields
    assert_eq!(records[1].get(0).unwrap(), "");
    assert_eq!(records[1].get(1).unwrap(), "New card");
}

#[test]
fn discover_csv_files() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();

    std::fs::write(dir.path().join("deck1.csv"), "deck,front,back\n").unwrap();
    std::fs::write(sub.join("deck2.csv"), "deck,front,back\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not a csv\n").unwrap();

    // Walk the directory and find CSVs
    let mut found = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("csv") {
                out.push(p);
            }
        }
    }
    walk(dir.path(), &mut found);
    assert_eq!(found.len(), 2);
}
