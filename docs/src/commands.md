# Commands

### `repeater drill [PATH ...]`

Start a terminal drilling session for one or more files/directories (default: current directory).

- `--card-limit <N>`: cap the number of cards reviewed this session.
- `--new-card-limit <N>`: cap the number of unseen cards introduced.
- `--rephrase`: rephrase basic questions via the LLM helper before the session starts.
- `--shuffle`: randomize the order of cards in the session.
- `--retention <FLOAT>`: target recall probability for FSRS scheduling (default: `0.9`, allowed range: `0.65`–`1.0`).

Example: drill all the physics decks and a single chemistry deck, stopping after 20 cards. This is just for extra practice, so let's lower the retention rate to `0.7`.

```sh
repeater drill flashcards/science/physics/ flashcards/science/chemistry.md --card-limit 20 --retention .7
```

Key bindings inside the drill UI:

- `Space`/`Enter`: reveal the answer or cloze.
- `F`: mark as `Fail`, `Space`/`Enter`: mark as `Pass`.
- `E`: edit the current card in-place and keep its scheduling history.
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

- `--plain`: print a plain-text summary to stdout instead of launching the dashboard.

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

### `repeater llm [--set|--clear|--test]`

Manage the optional LLM helper that can auto-cloze missing brackets and rephrase questions before a drill.

- `--set`: configure provider, base URL, API key, and model (stored in the local auth file).
- `--test`: verify the configured provider by listing models.
- `--clear`: delete the stored config; use this when rotating credentials.

Instead of `--set`, you can export `OPENAI_API_KEY` for one-off runs. Skip configuring this command entirely to keep the feature disabled.
