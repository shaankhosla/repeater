# Commands

### `repeater drill [PATH ...]`

Start a terminal drilling session for one or more files/directories (default: current directory).

- `--card-limit <N>`: cap the number of cards reviewed this session.
- `--new-card-limit <N>`: cap the number of unseen cards introduced.
- `--rephrase`: rephrase basic questions via the LLM helper before the session starts.
- `--shuffle`: randomize the order of cards in the session.
- `--retention <FLOAT>`: target recall probability for FSRS scheduling (default: `0.9`, allowed range: `0.65`–`1.0`).
- `--apple-notes` *(beta)*: source cards from Apple Notes instead of local Markdown files. macOS only — requires Full Disk Access for your terminal (System Settings > Privacy & Security > Full Disk Access). Conflicts with `[PATH ...]`.

Example: drill all the physics decks and a single chemistry deck, stopping after 20 cards. This is just for extra practice, so let's lower the retention rate to `0.7`.

```sh
repeater drill flashcards/science/physics/ flashcards/science/chemistry.md --card-limit 20 --retention .7
```

Key bindings inside the drill UI:

- `Space`/`Enter`: reveal the answer or cloze.
- `F`: mark as `Fail`, `Space`/`Enter`: mark as `Pass`.
- `O`: open the first media file detected in the current card (images/audio/video). The file opens in your OS default viewer before the answer is revealed.
- `Esc` / `Ctrl+C`: exit the session.

### `repeater drill-session <COMMAND>`

Drive reviews from agents and scripts through a JSON-only state machine. Successful
responses are written to stdout; application errors are JSON on stderr with a nonzero
exit status.

```sh
repeater drill-session start cards/ --retention 0.9
repeater drill-session next <SESSION_ID>
repeater drill-session reveal <REVIEW_ID>
repeater drill-session mark <REVIEW_ID> pass
```

- `start [PATH ...]`: store absolute source paths and immutable session settings,
  then return a session token. Supports `--apple-notes`, `--rephrase`,
  `--retention`, and `--shuffle`.
- `next <SESSION_ID>`: rescan the configured source and return one due question.
  Calling it again before reveal returns the same review.
- `reveal <REVIEW_ID>`: return the snapshotted answer. Repeated calls are
  idempotent.
- `mark <REVIEW_ID> <pass|fail>`: update FSRS and return the resulting due date.
  Repeating the same result does not schedule the card twice.

Each card presentation has a separate review token so retries cannot mark a later
card accidentally. Sessions expire after 24 hours. The answer is omitted from
`next`; it is persisted locally so source edits between `next` and `reveal` cannot
change an active review.

The queue is live rather than frozen: each `next` scans the source again. When
nothing is due, the session returns `state: "complete"`.


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
- `--apple-notes` *(beta)*: source cards from Apple Notes instead of local Markdown files. macOS only — requires Full Disk Access for your terminal. Conflicts with `[PATH ...]`.

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
