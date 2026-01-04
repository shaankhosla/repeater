# FAQ

## How is scheduling different from Anki?

`repeat` schedules cards with the Free Spaced Repetition Scheduler (FSRS) targeting ~90 % recall, so you get dynamically computed intervals instead of SM-2’s fixed ease multipliers. Inside the drill UI there are only two buttons—`Pass` (`2`) and `Fail` (`1`)—which the code maps to FSRS quality scores of 3 and 1, respectively, while still applying the upstream stability/difficulty math plus the short “learning” ramp for your first few reviews. The end result feels faster to grade yet still reuses FSRS’s predictions.

## Where does my progress live?

Your decks stay in plain Markdown wherever you save them, but progress metadata (stability, difficulty, due dates, etc.) is tracked in `cards.db` under the platform’s application data directory (for example `~/Library/Application Support/repeat/cards.db` on macOS). Back up or sync that file if you want to keep review history when moving machines; deleting it resets scheduling without touching the Markdown decks.

## What happens if I edit or move a card?

Every card is hashed based on its text, so edits produce a new hash and therefore start a fresh review history. This makes refactors safe—old records are ignored after their content disappears—but also means typos fixes reset their interval, so bundle small edits if you care about keeping streaks. Just moving blocks between files will not reset review history since the content of the card will be the same

## Can I study ahead or repeat lapses immediately?

Yes. Anything due within the next 20 minutes is considered “due now”, so `repeat check` will show it and drills will surface it alongside overdue cards. During a session, cards that fail or return ultra-short intervals (under that 20‑minute window) are automatically added back into the current queue so you can clear them before quitting.

## Does the Anki import carry over scheduling data?

`repeat import` converts `.apkg` exports into Markdown decks today, but it does not migrate Anki’s per-card FSRS/SM-2 history yet. Imported notes will be treated as new cards and scheduled fresh once they’re indexed.

## I’m a developer—what’s the quickest way to run checks locally?

Use the `just precommit` recipe to run `cargo fmt`, `cargo clippy --fix`, `cargo machete`, and the full test suite behind `SQLX_OFFLINE=true`. The Justfile also includes helper recipes for launching `repeat create`, `check`, `drill`, and `import` against sample data plus the release workflow, so contributors can rely on those instead of memorizing individual cargo commands.

---

Still stuck or ran into a bug? Please open an issue at [github.com/shaankhosla/repeat/issues](https://github.com/shaankhosla/repeat/issues) with logs and repro steps so we can help.
