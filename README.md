# repeat

<p align="center">
  <a href="https://github.com/shaankhosla/repeat/actions/workflows/ci.yaml">
    <img alt="CI Status" src="https://img.shields.io/github/actions/workflow/status/shaankhosla/repeat/ci.yaml?branch=main&label=CI&logo=github" />
  </a>
  <a href="https://github.com/shaankhosla/repeat/releases">
    <img alt="Latest Release" src="https://img.shields.io/github/v/release/shaankhosla/repeat?display_name=tag&sort=semver&logo=github" />
  </a>
  <a href="LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/shaankhosla/repeat?color=informational" />
  </a>
</p>

`repeat` is a command-line flashcard program that uses spaced repetition to boost your memory retention. It’s like a lightweight, text-based Anki you run in your terminal. Your decks are kept in Markdown, progress is tracked in SQLite, and reviewes are scheduled with Free Spaced Repetition Scheduler (FSRS), a state-of-the-art algorithm targeting 90% recall.

<p align="center">
  <img src="create_example.png" alt="Creating cards in the built-in editor" width="45%" />
  <img src="check_example.png" alt="Checking card progress" width="45%" />
</p>

## Features

- Plain-text decks: recurse through directories of `.md` files; each `Q:/A:` or `C:` block is parsed into a flashcard.
- Content-addressed cards: cards are keyed by a Blake3 hash of their text, so edits automatically reset their progress.
- FSRS scheduling: intervals, stability, and difficulty are recalculated on every review and stored in SQLite.
- Terminal UX: `repeat drill` renders cards with ratatui; `repeat create` launches an editor dedicated to card capture.
- Progress at a glance: `repeat check` prints totals, due/overdue counts, and a 7-day due histogram.

## Installation

### Install script (macOS + Linux)

Use the included `install.sh` to grab the latest GitHub release for your platform, verify its checksum, and place the binary in `/usr/local/bin` (you may be prompted for sudo):

```
curl -fsSL https://raw.githubusercontent.com/shaankhosla/repeat/main/install.sh | bash
```

### Homebrew (macOS + Linux)

```
brew tap shaankhosla/homebrew-tap
brew install repeat
```

## Quick Start

1. Create a deck in Markdown (`cards/neuro.md`):

   ```markdown
   Q: What does a synaptic vesicle store?
   A: Neurotransmitters awaiting release.

   C: Speech is [produced] in [Broca's] area.
   ```

2. Index the cards and start a session:

   ```
   repeat drill cards
   ```

   - `Space`/`Enter`: reveal the answer or cloze.
   - `1`: mark as `Fail`, `2`: mark as `Pass`.
   - `Esc` or `Ctrl+C`: end the session early (progress so far is saved).

3. Check your collection status:

   ```
   repeat check cards
   ```

   The command prints totals for new/reviewed cards, due/overdue counts, and upcoming workload.

## Card Format
Files can be structured in any way, such as:

```
flashcards/
  math.md
  science/
      physics.md
      chemistry.md
      ...
```

Cards live in ordinary Markdown. `repeat` scans for tagged sections and turns them into flashcards.

- **Basic cards**

  ```
  Q: What is Coulomb's constant?
  A: The proportionality constant of the electric force.
  ```

- **Cloze cards**

  ```
  C: The [order] of a group is [the cardinality of its underlying set].
  ```

Multi-line content is supported.

## Commands

### `repeat drill [PATH ...]`

Start a terminal drilling session for one or more files/directories (default: current directory). Options:

- `--card-limit <N>`: cap the number of cards reviewed this session.
- `--new-card-limit <N>`: cap the number of unseen cards introduced.

### `repeat create <path/to/deck.md>`

Launch the capture editor for a specific Markdown file (it is created if missing). Shortcuts:

- `Ctrl+B`: start a basic (`Q:/A:`) template.
- `Ctrl+K`: start a cloze (`C:`) template.
- `Ctrl+S`: save the current card (hash collisions are rejected).
- Arrow keys/PageUp/PageDown: move the cursor; `Tab`, `Enter`, `Backspace`, and `Delete` work as expected.
- `Esc` or `Ctrl+C`: exit the editor.

### `repeat check [PATH ...]`

Re-index the referenced decks and emit counts for total, new, due, overdue, and upcoming cards so you can gauge the workload before drilling.

## Data & Scheduling

- Cards are hashed with Blake3; modifying the text produces a new hash and resets that card's performance history.
- Scheduling uses FSRS weights defined in `src/fsrs.rs`, clamping intervals between 1 and 256 days and targeting 90% recall.
- Metadata lives in `cards.db` under your OS data directory (for example, `~/Library/Application Support/repeat/cards.db` on macOS). Delete the file to discard history; the Markdown decks remain untouched.

## Development

Run the lint/test suite with:

```
cargo fmt --all
cargo clippy
cargo test
```

The repository also ships a `just precommit` recipe that runs the same checks.


## Roadmap

- [ ] Import from Anki
- [ ] Allow scrolling to other cards in a collection while creating a new card
- [ ] Edit an existing card while keeping the progress intact
- [ ] Allow for a fuzzy search of existing cards
- [ ] Use LLMs to import from various content sources


## License

Licensed under the Apache License, Version 2.0. See `LICENSE` for details.
