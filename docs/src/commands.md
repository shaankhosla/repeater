# Commands

### `repeater drill [PATH ...]`

Start a terminal drilling session for one or more files/directories (default: current directory).

- `--card-limit <N>`: cap the number of cards reviewed this session.
- `--new-card-limit <N>`: cap the number of unseen cards introduced.

Example: drill all the physics decks and a single chemistry deck, stopping after 20 cards.

```sh
repeater drill flashcards/science/physics/ flashcards/science/chemistry.md --card-limit 20
```

Key bindings inside the drill UI:

- `Space`/`Enter`: reveal the answer or cloze.
   - `F`: mark as `Fail`, `Space`/`Enter`: mark as `Pass`.
- `O`: open the first media file detected in the current card (images/audio/video). The file opens in your OS default viewer before the answer is revealed.
- `Esc` / `Ctrl+C`: exit the session.

### `repeater create <path/to/deck.md>`

Launch the capture editor for a specific Markdown file (it is created if missing).

- `Ctrl+B`: start a basic (`Q:/A:`) template.
- `Ctrl+K`: start a cloze (`C:`) template.
- `Ctrl+S`: save the current card; you’ll be warned if another card already uses the same meaningful text.
- Arrow keys/PageUp/PageDown: move the cursor; `Tab`, `Enter`, `Backspace`, and `Delete` work as expected.
- `Esc` or `Ctrl+C`: exit the editor.

Example:

```sh
repeater create cards/neuro.md
```

### `repeater check [PATH ...]`

Re-index the referenced decks and open the interactive dashboard with totals for new, due, overdue, and upcoming cards (press `Esc`/`Ctrl+C` to exit).

Example:

```sh
repeater check flashcards/math/
```

### `repeater import <anki.apkg> <output-dir>`

Convert an Anki `.apkg` export into Markdown decks. Existing files in the export folder are overwritten, so rerunning is safe. FSRS history is not yet transferred.

Example:

```sh
repeater import ~/Downloads/my_collection.apkg cards/anki
```
