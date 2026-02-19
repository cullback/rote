use chrono::NaiveDate;

use crate::card::{self, Card};
use crate::fsrs::{self, Grade};

pub struct ReviewItem {
    pub card_index: usize,
    pub front_display: String,
    pub reveal_display: String,
    pub deck: String,
}

pub struct DeckSummary {
    pub name: String,
    pub total: usize,
    pub due: usize,
}

pub fn render_front(text: &str) -> String {
    let clozes = card::extract_cloze_deletions(text);
    if clozes.is_empty() {
        card::expand_newlines(text)
    } else {
        let mut result = text.to_string();
        // Replace each [...] with _____
        let mut out = String::new();
        let mut in_bracket = 0usize;
        for ch in result.chars() {
            match ch {
                '[' => {
                    if in_bracket == 0 {
                        out.push_str("_____");
                    }
                    in_bracket += 1;
                }
                ']' => {
                    in_bracket = in_bracket.saturating_sub(1);
                }
                _ => {
                    if in_bracket == 0 {
                        out.push(ch);
                    }
                }
            }
        }
        result = out;
        card::expand_newlines(&result)
    }
}

pub fn render_reveal(front: &str, back: &str) -> String {
    // Remove brackets but keep content
    let mut full_front = String::new();
    for ch in front.chars() {
        if ch != '[' && ch != ']' {
            full_front.push(ch);
        }
    }
    let full_front = card::expand_newlines(&full_front);
    let back = card::expand_newlines(back);

    if back.trim().is_empty() {
        full_front
    } else {
        format!("{full_front}\n---\n{back}")
    }
}

pub fn build_review_items(cards: &[Card], indices: &[usize]) -> Vec<ReviewItem> {
    indices
        .iter()
        .map(|&i| {
            let card = &cards[i];
            ReviewItem {
                card_index: i,
                front_display: render_front(&card.front),
                reveal_display: render_reveal(&card.front, &card.back),
                deck: card.deck.clone(),
            }
        })
        .collect()
}

pub fn filter_due(cards: &[Card], today: NaiveDate) -> Vec<usize> {
    cards
        .iter()
        .enumerate()
        .filter(|(_, card)| match card.due {
            None => true, // new card
            Some(due) => due <= today,
        })
        .map(|(i, _)| i)
        .collect()
}

pub fn deck_summaries(cards: &[Card], today: NaiveDate) -> Vec<DeckSummary> {
    let mut decks: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for card in cards {
        let entry = decks.entry(card.deck.clone()).or_insert((0, 0));
        entry.0 += 1;
        let is_due = match card.due {
            None => true,
            Some(due) => due <= today,
        };
        if is_due {
            entry.1 += 1;
        }
    }
    decks
        .into_iter()
        .map(|(name, (total, due))| DeckSummary { name, total, due })
        .collect()
}

pub fn apply_grade(card: &mut Card, grade: Grade, today: NaiveDate) {
    let outcome =
        if card.stability.is_some() && card.difficulty.is_some() && card.last_review.is_some() {
            let days_elapsed = (today - card.last_review.unwrap()).num_days() as f64;
            let days_elapsed = if days_elapsed < 0.0 {
                0.0
            } else {
                days_elapsed
            };
            fsrs::review_existing(
                card.difficulty.unwrap(),
                card.stability.unwrap(),
                days_elapsed,
                grade,
                today,
            )
        } else {
            fsrs::review_new(grade, today)
        };

    card.stability = Some(outcome.stability);
    card.difficulty = Some(outcome.difficulty);
    card.due = Some(outcome.due);
    card.last_review = Some(today);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_front_no_cloze() {
        assert_eq!(render_front("What is 2+2?"), "What is 2+2?");
    }

    #[test]
    fn render_front_with_cloze() {
        assert_eq!(
            render_front("The [mitochondria] is the [powerhouse] of the cell"),
            "The _____ is the _____ of the cell"
        );
    }

    #[test]
    fn render_front_expands_newlines() {
        assert_eq!(render_front("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn render_reveal_with_back() {
        let result = render_reveal("[mitochondria]", "organelle");
        assert_eq!(result, "mitochondria\n---\norganelle");
    }

    #[test]
    fn render_reveal_no_back() {
        let result = render_reveal("[mitochondria]", "");
        assert_eq!(result, "mitochondria");
    }

    #[test]
    fn filter_due_new_cards() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let cards = vec![Card {
            deck: "test".into(),
            front: "q".into(),
            back: "a".into(),
            media: String::new(),
            id: "1".into(),
            stability: None,
            difficulty: None,
            due: None,
            last_review: None,
        }];
        let due = filter_due(&cards, today);
        assert_eq!(due, vec![0]);
    }

    #[test]
    fn filter_due_past_due() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 10).unwrap();
        let cards = vec![Card {
            deck: "test".into(),
            front: "q".into(),
            back: "a".into(),
            media: String::new(),
            id: "1".into(),
            stability: Some(3.0),
            difficulty: Some(5.0),
            due: NaiveDate::from_ymd_opt(2025, 6, 5),
            last_review: NaiveDate::from_ymd_opt(2025, 6, 1),
        }];
        let due = filter_due(&cards, today);
        assert_eq!(due, vec![0]);
    }

    #[test]
    fn filter_due_not_yet() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let cards = vec![Card {
            deck: "test".into(),
            front: "q".into(),
            back: "a".into(),
            media: String::new(),
            id: "1".into(),
            stability: Some(3.0),
            difficulty: Some(5.0),
            due: NaiveDate::from_ymd_opt(2025, 6, 10),
            last_review: NaiveDate::from_ymd_opt(2025, 6, 1),
        }];
        let due = filter_due(&cards, today);
        assert!(due.is_empty());
    }

    #[test]
    fn apply_grade_new_card() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let mut card = Card {
            deck: "test".into(),
            front: "q".into(),
            back: "a".into(),
            media: String::new(),
            id: "1".into(),
            stability: None,
            difficulty: None,
            due: None,
            last_review: None,
        };
        apply_grade(&mut card, Grade::Good, today);
        assert!(card.stability.is_some());
        assert!(card.difficulty.is_some());
        assert!(card.due.is_some());
        assert_eq!(card.last_review, Some(today));
        assert!(card.due.unwrap() > today);
    }

    #[test]
    fn apply_grade_existing_card() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let mut card = Card {
            deck: "test".into(),
            front: "q".into(),
            back: "a".into(),
            media: String::new(),
            id: "1".into(),
            stability: Some(3.173),
            difficulty: Some(5.5),
            due: Some(today),
            last_review: NaiveDate::from_ymd_opt(2025, 5, 28),
        };
        let old_stability = card.stability.unwrap();
        apply_grade(&mut card, Grade::Good, today);
        assert!(card.stability.unwrap() > old_stability);
        assert!(card.due.unwrap() > today);
    }

    #[test]
    fn deck_summaries_grouping() {
        let today = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
        let cards = vec![
            Card {
                deck: "math".into(),
                front: "q1".into(),
                back: "a1".into(),
                media: String::new(),
                id: "1".into(),
                stability: None,
                difficulty: None,
                due: None,
                last_review: None,
            },
            Card {
                deck: "math".into(),
                front: "q2".into(),
                back: "a2".into(),
                media: String::new(),
                id: "2".into(),
                stability: Some(3.0),
                difficulty: Some(5.0),
                due: NaiveDate::from_ymd_opt(2025, 7, 1),
                last_review: Some(today),
            },
            Card {
                deck: "science".into(),
                front: "q3".into(),
                back: "a3".into(),
                media: String::new(),
                id: "3".into(),
                stability: None,
                difficulty: None,
                due: None,
                last_review: None,
            },
        ];
        let summaries = deck_summaries(&cards, today);
        assert_eq!(summaries.len(), 2);
        let math = summaries.iter().find(|s| s.name == "math").unwrap();
        assert_eq!(math.total, 2);
        assert_eq!(math.due, 1);
        let science = summaries.iter().find(|s| s.name == "science").unwrap();
        assert_eq!(science.total, 1);
        assert_eq!(science.due, 1);
    }
}
