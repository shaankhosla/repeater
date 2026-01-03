# Usage

## Card Format

Store decks anywhere, for example:

```
flashcards/
  math.md
  science/
      physics.md
      chemistry.md
```

Cards live in everyday Markdown. `repeat` scans for tagged sections and turns them into flashcards, so you can mix active-recall prompts with your normal notes.

- **Basic cards**

  ```markdown
  Q: What is Coulomb's constant?
  A: The proportionality constant of the electric force.
  ```

- **Cloze cards**

  ```markdown
  C: The [order] of a group is [the cardinality of its underlying set].
  ```

### Parsing Logic

- Cards are detected by the presence of a `Q:/A:` or `C:` block. A horizontal rule (`---`) or the start of another card marks the end.
- Cards are hashed with Blake3; editing the text resets the card's performance history.
- Metadata lives in `cards.db` under your OS data directory (for example, `~/Library/Application Support/repeat/cards.db` on macOS). Delete this file to reset history; the Markdown decks remain untouched.
- Multi-line content is supported.

## Commands

### `repeat drill [PATH ...]`

Start a terminal drilling session for one or more files/directories (default: current directory).

- `--card-limit <N>`: cap the number of cards reviewed this session.
- `--new-card-limit <N>`: cap the number of unseen cards introduced.

Example: drill all the physics decks and a single chemistry deck, stopping after 20 cards.

```sh
repeat drill flashcards/science/physics/ flashcards/science/chemistry.md --card-limit 20
```

### `repeat create <path/to/deck.md>`

Launch the capture editor for a specific Markdown file (it is created if missing).

- `Ctrl+B`: start a basic (`Q:/A:`) template.
- `Ctrl+K`: start a cloze (`C:`) template.
- `Ctrl+S`: save the current card (hash collisions are rejected).
- Arrow keys/PageUp/PageDown: move the cursor; `Tab`, `Enter`, `Backspace`, and `Delete` work as expected.
- `Esc` or `Ctrl+C`: exit the editor.

Example:

```sh
repeat create cards/neuro.md
```

### `repeat check [PATH ...]`

Re-index the referenced decks and emit counts for total, new, due, overdue, and upcoming cards.

Example:

```sh
repeat check flashcards/math/
```

### `repeat import <anki.apkg> <output-dir>`

Convert an Anki `.apkg` export into Markdown decks. Existing files in the export folder are overwritten, so rerunning is safe. FSRS history is not yet transferred.

Example:

```sh
repeat import ~/Downloads/my_collection.apkg cards/anki
```
