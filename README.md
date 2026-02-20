# Rote

A plain-text spaced repetition system. You author flashcards in CSV files, and Rote handles scheduling using the [FSRS](https://github.com/open-spaced-repetition/fsrs4anki) algorithm.

## Install

```
# With Nix
nix profile install github:cullback/rote

# Or build from source
cargo install --path .
```

## Usage

Create a CSV file with your cards:

```csv
deck,front,back,media,id,stability,difficulty,due,last_review
science,The human body contains approximately [206] bones,At birth we have ~270 many fuse together during development,,,,,,
science,Light takes approximately [8 minutes] to travel from the Sun to Earth,The distance is 149.6 million km ($1$ AU),,,,,,
history,The shortest war in recorded history lasted [38 minutes],The Anglo-Zanzibar War of 1896,,,,,,
```

You write the first four columns (`deck`, `front`, `back`, `media`). Leave the rest empty — Rote fills them in on first review.

Then drill:

```
rote drill cards.csv
```

```
Decks:
  1: history (1 due / 1 total)
  2: science (2 due / 2 total)
  0: All decks

Select deck(s) (comma-separated numbers, or 0 for all): 0

3 cards due for review.

[1/3] science

The human body contains approximately _____ bones

Press Enter to reveal...

The human body contains approximately 206 bones
---
At birth we have ~270; many fuse together during development

Rate (1=forgot, 2=hard, 3=good, 4=easy): 3
```

After the session, your CSV is updated in place with scheduling state. Run `rote drill` again tomorrow and only due cards appear.

## Features

- **CSV as the database** — cards are plain text files you can edit, diff, grep, and version control
- **FSRS scheduling** — the same algorithm replacing SM-2 in Anki, giving ~30% less review time for the same retention
- **Cloze deletions** — wrap terms in `[brackets]` and they're blanked during review
- **LaTeX and Markdown** — use `$...$` or `$$...$$` in card content, rendered as-is in the terminal
- **Multi-file, multi-deck** — pass files and directories to `drill`; deck grouping is by the `deck` column, not by file
- **Zero config** — no database, no account, no sync service; just CSV files and a binary

## Rationale

### Why CSV, not Markdown?

A flashcard is two sentences of text in 95% of cases. Markdown doesn't buy you much here, and bundling multiple cards in one Markdown file requires a custom parser for delimiters, frontmatter, and embedded scheduling state. CSV lets you see all your cards at a glance in any spreadsheet editor, which is what you want when authoring or editing a topic.

### Why store scheduling state in the same file?

One file per deck means one thing to back up, one thing to sync, and one thing to version control. There's no separate database to keep in sync with your cards. `git diff` after a session shows exactly which cards you reviewed and how the schedule changed.
