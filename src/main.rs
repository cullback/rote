use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use rote::{card, fsrs, review};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: rote <command> [args...]");
        eprintln!("Commands:");
        eprintln!("  drill <paths...>            Review cards in the terminal");
        eprintln!("  serve <paths...> [-p PORT]   Start web UI (default port 3000)");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "drill" => {
            if args.len() < 3 {
                eprintln!("Usage: rote drill <paths...>");
                std::process::exit(1);
            }
            drill(&args[2..]);
        }
        "serve" => {
            if args.len() < 3 {
                eprintln!("Usage: rote serve <paths...> [-p PORT]");
                std::process::exit(1);
            }
            let (paths, port) = parse_serve_args(&args[2..]);
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(rote::web::serve(paths, port));
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Commands: drill, serve");
            std::process::exit(1);
        }
    }
}

fn parse_serve_args(args: &[String]) -> (Vec<String>, u16) {
    let mut paths = Vec::new();
    let mut port = 3000u16;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-p" && i + 1 < args.len() {
            port = args[i + 1].parse().unwrap_or_else(|_| {
                eprintln!("Invalid port: {}", args[i + 1]);
                std::process::exit(1);
            });
            i += 2;
        } else {
            paths.push(args[i].clone());
            i += 1;
        }
    }
    (paths, port)
}

fn drill(args: &[String]) {
    let files = card::discover_files(args);
    if files.is_empty() {
        eprintln!("No CSV files found.");
        std::process::exit(1);
    }

    // Load all cards, tracking source file per card
    let mut all_cards: Vec<card::Card> = Vec::new();
    let mut card_source: Vec<PathBuf> = Vec::new();

    for file in &files {
        match card::load_csv(file) {
            Ok(cards) => {
                for c in cards {
                    card_source.push(file.clone());
                    all_cards.push(c);
                }
            }
            Err(e) => {
                eprintln!("Warning: {e}");
            }
        }
    }

    if all_cards.is_empty() {
        eprintln!("No cards found.");
        std::process::exit(1);
    }

    let today = chrono::Local::now().date_naive();

    // Show deck summaries
    let summaries = review::deck_summaries(&all_cards, today);
    println!("Decks:");
    for (i, s) in summaries.iter().enumerate() {
        println!(
            "  {}: {} ({} due / {} total)",
            i + 1,
            s.name,
            s.due,
            s.total
        );
    }
    println!("  0: All decks");
    println!();

    // Prompt for selection
    let selected_decks = prompt_deck_selection(&summaries);

    // Filter to due cards in selected decks
    let due_indices = review::filter_due(&all_cards, today);
    let due_in_selected: Vec<usize> = due_indices
        .into_iter()
        .filter(|&i| selected_decks.is_empty() || selected_decks.contains(&all_cards[i].deck))
        .collect();

    if due_in_selected.is_empty() {
        println!("No cards due for review.");
        return;
    }

    println!("{} cards due for review.\n", due_in_selected.len());

    // Build review items and shuffle
    let mut items = review::build_review_items(&all_cards, &due_in_selected);
    shuffle(&mut items);

    // Drill loop
    let mut counts = [0u32; 4]; // forgot, hard, good, easy
    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    for (i, item) in items.iter().enumerate() {
        println!("[{}/{}] {}", i + 1, items.len(), item.deck);
        println!();
        println!("{}", item.front_display);
        println!();

        // Wait for Enter to reveal
        print!("Press Enter to reveal...");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        stdin.read_line(&mut buf).unwrap();

        println!("{}", item.reveal_display);
        println!();

        // Get rating
        let grade = loop {
            print!("Rate (1=forgot, 2=hard, 3=good, 4=easy): ");
            io::stdout().flush().unwrap();
            buf.clear();
            stdin.read_line(&mut buf).unwrap();
            if let Ok(n) = buf.trim().parse::<u8>()
                && let Some(g) = fsrs::Grade::from_u8(n)
            {
                break g;
            }
            println!("Please enter 1, 2, 3, or 4.");
        };

        let grade_idx = match grade {
            fsrs::Grade::Forgot => 0,
            fsrs::Grade::Hard => 1,
            fsrs::Grade::Good => 2,
            fsrs::Grade::Easy => 3,
        };
        counts[grade_idx] += 1;

        review::apply_grade(&mut all_cards[item.card_index], grade, today);
        println!();
    }

    // Save all cards back to their source files
    let mut files_to_save: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (i, source) in card_source.iter().enumerate() {
        files_to_save.entry(source.clone()).or_default().push(i);
    }

    for (path, indices) in &files_to_save {
        let file_cards: Vec<card::Card> = indices.iter().map(|&i| all_cards[i].clone()).collect();
        if let Err(e) = card::save_csv(path, &file_cards) {
            eprintln!("Error saving {}: {e}", path.display());
        }
    }

    // Session summary
    println!("Session complete!");
    println!(
        "  Forgot: {}, Hard: {}, Good: {}, Easy: {}",
        counts[0], counts[1], counts[2], counts[3]
    );
}

fn prompt_deck_selection(summaries: &[review::DeckSummary]) -> Vec<String> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    loop {
        print!("Select deck(s) (comma-separated numbers, or 0 for all): ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        stdin.read_line(&mut buf).unwrap();

        let mut selected = Vec::new();
        let mut valid = true;

        for part in buf.trim().split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.parse::<usize>() {
                Ok(0) => return Vec::new(), // all decks
                Ok(n) if n >= 1 && n <= summaries.len() => {
                    selected.push(summaries[n - 1].name.clone());
                }
                _ => {
                    valid = false;
                    break;
                }
            }
        }

        if valid && !selected.is_empty() {
            return selected;
        }
        println!("Invalid selection. Try again.");
    }
}

fn shuffle<T>(items: &mut [T]) {
    // Simple Fisher-Yates using a basic seeded RNG (xorshift64)
    let mut state: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    if state == 0 {
        state = 1;
    }

    for i in (1..items.len()).rev() {
        // xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        items.swap(i, j);
    }
}
