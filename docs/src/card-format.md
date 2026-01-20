# Card Format

Store decks anywhere, for example:

```
flashcards/
  math.md
  science/
      physics.md
      chemistry.md
```

Cards live in everyday Markdown. `repeater` scans for tagged sections and turns them into flashcards, so you can mix active-recall prompts with your normal notes.

- **Basic cards**

  ```markdown
  Q: What is Coulomb's constant?
  A: The proportionality constant of the electric force.
  ```

  Multi-line basic:

  ```markdown
  Q: List the SI base units.
  A: meter (m)
  kilogram (kg)
  second (s)
  ```

  Single-line variant:

  ```markdown
  What is Coulomb's constant?::The proportionality constant of the electric force.
  ```

- **Cloze cards**

  ```markdown
  C: The [order] of a group is [the cardinality of its underlying set].
  ```

## Parsing Logic

- Cards are detected by the presence of `Q:/A:`, `C:`, or `::`. A horizontal rule (`---`) or the start of another card marks the end.
- Lines with `::` are treated as single-line basic cards (left side = question, right side = answer).
- Each card gets a hash (think fingerprint) built from its letters, numbers, and any `+`/`-` signs. Punctuation, spacing, and capitalization are ignored, so only meaningful text changes create a new history.
- Metadata lives in `cards.db` under your OS data directory (for example, `~/Library/Application Support/repeater/cards.db` on macOS). Delete this file to reset history; the Markdown decks remain untouched.
- Multi-line content is supported.

### Edge case examples

- **Markers must start at column 0.** Indented `Q:`, `C:`, or `---` lines are ignored by the scanner, so the snippet below produces zero cards.
  ```markdown
    Q: Skipped
    A: Because the tag is indented
  ```
- **Next marker auto-closes the previous card.** A new `Q:` or `C:` flushes the current buffer even without `---`.
  ```markdown
  Q: First?
  A: Ends here
  Q: Second starts now
  ```
- **Notes need a separator.** Without a flush-left `---`, trailing notes remain part of the last card.
  ```markdown
  Q: Term?
  A: Definition
  This line still belongs to the answer
  ```
- **Basic cards require both tags.** Missing or blank `Q:`/`A:` blocks throw a parse error for that card.
  ```markdown
  Q: What is ATP?
  ---  ← rejected; no answer was captured
  ```
- **Single-line cards require both sides.** Blank answers are rejected.
  ```markdown
  What is ATP?::
  ```
- **Cloze blocks need real `[hidden]` text.** Empty brackets or unmatched `[`/`]` abort parsing.
  ```markdown
  C: Bad []    ← rejected
  C: Half [good   ← rejected
  ```
