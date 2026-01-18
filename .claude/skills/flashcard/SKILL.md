# /flashcard - PDF to Flashcard Generator

Generate source-verified flashcards from PDF materials for the repeater spaced repetition system.

**Topic-specific guidance:** Check `.claude/skills/flashcard/topics/` for domain-specific patterns (sermons, textbooks, research papers, etc.).

## Invocation

```
/flashcard                      # Interactive mode
/flashcard <path>               # With source file
/flashcard cards/raw/topic.pdf  # Common usage
```

---

## PHASE 1: REQUIREMENTS GATHERING

### Step 1: Identify Source File

Check for source file in this order:
1. If path argument provided, use it
2. Scan `cards/raw/` for recent PDFs, offer selection
3. Prompt user for path

**Auto-detection prompt:**
```
Source file?

Found in cards/raw/:
  [1] biology/chapter-05-cells.pdf (342 KB, modified today)
  [2] history/lecture-wwii.pdf (298 KB, modified yesterday)
  [3] Enter different path

Select:
```

**Validate file:**
- Must exist and be readable
- Must be .pdf or .txt
- If PDF, verify `pdftotext` available: `which pdftotext`

### Step 2: Determine Target Deck

Scan `cards/` for existing .md files:
```
Target deck?

Existing decks:
  [1] biology-cells.md (34 cards)
  [2] world-history.md (28 cards)
  [3] Create new deck

Select:
```

**If augmenting existing deck:**
- Read existing deck to extract card hashes for duplicate detection
- Note insertion point (append to end)
- Will add timestamp comment: `# --- Added YYYY-MM-DD ---`

**If creating new deck:**
- Prompt for deck name
- Create as `cards/<name>.md`

### Step 3: Confirm and Proceed

```
Ready to generate:
  Source: cards/raw/biology/chapter-05-cells.pdf
  Target: cards/biology-cells.md (augment, 34 existing cards)

Proceed? [y/n]
```

---

## PHASE 2: EXTRACT + GENERATE

### Step 1: Extract PDF Text

```bash
pdftotext -layout "<input.pdf>" /tmp/flashcard_source.txt
```

If extraction fails:
- Check if poppler-utils installed
- Offer: "Provide a .txt file instead?"

Read extracted text into context.

### Step 2: Parse Document Structure

**Check for topic-specific guidance:** If source type is recognized (sermon, textbook, research paper, etc.), consult `.claude/skills/flashcard/topics/<type>.md` for specialized parsing patterns.

Identify:
- **Title:** First major heading or document title
- **Sections:** Headers, numbered points, outline structure
- **Page breaks:** Form feed characters or clear breaks
- **Research boundary:** Look for "Research", "Notes", "References" section

Build mental map:
```
Document: "[Document Title/Topic]"
├── Introduction/Overview (page 1)
├── Section 1: [Main Topic] (pages 2-5)
│   ├── 1.A: [Subtopic]
│   └── 1.B: [Subtopic]
├── Section 2: [Main Topic] (pages 6-10)
│   ├── 2.A: [Subtopic]
│   └── 2.B: [Subtopic]
├── Section 3: [Main Topic] (pages 11-12)
└── Notes/References/Appendix (pages 13-15) [SECONDARY]
```

### Step 3: Generate Cards

For each key teaching point, create a card. Apply the VERIFICATION CHECKLIST before writing each card.

**Target distribution:**
- ~1 card per 300-500 words of primary content
- Mix of Q&A (60-70%) and Cloze (30-40%)
- Cloze for: technical terms, specialized vocabulary, key definitions, important phrases

---

## VERIFICATION CHECKLIST (Per Card)

Before writing ANY card, verify ALL boxes:

### Factual Accuracy
```
□ Can I point to the EXACT sentence(s) in source that support this?
□ If paraphrasing, does it preserve the EXACT meaning?
□ Am I adding ANY external knowledge? (If yes → SKIP)
□ Am I making ANY inference? (If yes → SKIP)
```

### Source Traceability
```
□ Can I cite specific section + page number?
□ For Q&A: Can I include a direct quote in the citation?
□ For Cloze: Does my citation NOT reveal the bracketed answer?
```

### Format Compliance
```
□ Q:/A:/C: marker starts at column 0 (no indentation)
□ Cloze has EXACTLY ONE [bracket] pair
□ Card separator --- between cards
```

### Quality Check
```
□ Question has ONE clear, unambiguous answer
□ Answer is concise (doesn't repeat the question)
□ Cloze brackets the MOST important term (not arbitrary)
□ Card tests meaningful knowledge (not trivia)
```

---

## FLAGGING RULES

Flag a card for user review if ANY of these apply:

### FLAG: Synthesized Answer
The answer combines information from 2+ non-adjacent sentences.
```
Example: Answer requires combining page 3 paragraph 2 with page 7 paragraph 4
Action: Flag with note "Answer synthesized from multiple sections - verify intent"
```

### FLAG: Ambiguous Question
Multiple distinct answers could be correct from this source.
```
Example: "What does Paul say about grace?" (too broad)
Action: Flag with suggested revision to narrow scope
```

### FLAG: Paraphrased Content
Answer is not verbatim; meaning preserved but wording changed significantly.
```
Example: Source says "dead in trespasses", card says "spiritually dead in sins"
Action: Flag with note "Paraphrased - verify accuracy"
```

### FLAG: Supplementary Section Content
Card derives from notes/references/appendix section, not primary content.
```
Example: Detail from appendix not mentioned in main chapters
Action: Flag with note "From supplementary section - confirm inclusion"
```

### FLAG: Cloze Citation Risk
Citation text contains or strongly hints at the bracketed term.
```
Example: C: We are saved by [grace]. Citation mentions "grace"
Action: Flag, suggest removing quote from citation
```

### DO NOT GENERATE (Skip entirely)
- Cannot find supporting text in source
- Requires inference or assumption
- Would add external knowledge
- Trivial or non-educational content

---

## CARD FORMAT

### Q&A Card Template
```markdown
Q: [Question derived directly from source - specific, single answer]
A: [Answer stated in source - concise, complete]
   [Source: "exact quote from source" - Section X.Y, page N]
```

### Q&A Examples

**Good:**
```markdown
Q: According to the text, what are the three main components of a eukaryotic cell?
A: Nucleus, cytoplasm, and cell membrane.
   [Source: "All eukaryotic cells share three fundamental structures: a nucleus containing genetic material, cytoplasm filled with organelles, and a cell membrane regulating what enters and exits." - Section 2.A, page 6]
```

**Bad (too broad):**
```markdown
Q: What does the chapter say about cells?
A: Many things about cell structure...
```

**Bad (inference):**
```markdown
Q: Why do cells need membranes?
A: Because they need protection from the environment...
[This infers reasoning not stated in source]
```

### Cloze Card Template
```markdown
C: [Statement with ONE [key term] bracketed - term is most important concept]

(Section X.Y, page N)
```

### Cloze Examples

**Good:**
```markdown
C: The process of [mitosis] divides one cell into two genetically identical daughter cells.

(Section 1.A, page 3)
```

**Good:**
```markdown
C: The [phospholipid bilayer] forms the basic structure of all cell membranes.

(Section 2.A, page 6)
```

**Bad (reveals answer in citation):**
```markdown
C: Cells undergo [mitosis] during cell division.

> "During mitosis, the cell divides" (Section 1.B)
[Citation reveals the bracketed word!]
```

**Bad (arbitrary bracket):**
```markdown
C: The [cell] contains a nucleus.
[Brackets "cell" which is not the key concept being taught]
```

---

## DUPLICATE DETECTION

When augmenting an existing deck:

### Step 1: Hash Existing Cards
For each card in existing deck, compute normalized hash:
- Lowercase
- Remove punctuation
- Remove extra whitespace
- For Q&A: hash question text only
- For Cloze: hash full text with brackets removed

### Step 2: Check New Cards
Before adding each new card:
- Compute same normalized hash
- If hash matches existing card → Skip with note
- If hash is similar (>80% overlap) → Flag for user review

### Step 3: Report
```
Duplicate check:
  - 2 exact duplicates skipped
  - 1 similar card flagged for review

Similar card found:
  EXISTING: Q: What are the main components of a cell?
  NEW:      Q: According to the text, what structures make up a cell?

  [s]kip new / [k]eep both / [r]eplace existing
```

---

## PHASE 3: VERIFY + REVIEW

### Present Summary
```
Generation complete.

Cards created: 28
  - 18 Q&A cards
  - 10 Cloze cards

Verification:
  ✓ 24 cards: Ready (passed all checks)
  ⚠ 4 cards: Flagged for review

Duplicates:
  - 2 skipped (exact match)
  - 1 needs decision (similar)
```

### Interactive Review (Flagged Cards)

Present each flagged card with:
1. The card content
2. The specific issue
3. A suggested fix (if applicable)
4. Options: [a]ccept / [r]evise / [e]dit / [d]elete

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
FLAGGED 1/4: Ambiguous question
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Q: What does the text say about cell structure?
A: Cells have a nucleus, cytoplasm, and membrane.
   [Source: Section 1.A, page 3]

ISSUE: Question too broad - text says multiple things about cell structure.

SUGGESTED:
Q: What three components make up all eukaryotic cells according to Section 1.A?

[a]ccept original / [r]evise with suggestion / [e]dit manually / [d]elete
>
```

### Final Confirmation

After all flags reviewed:
```
Review complete.

Final deck: 27 cards
  - 24 auto-approved
  - 2 revised
  - 1 deleted

Save to cards/biology-cells.md? [y]es / [p]review all / [n]o
>
```

**If preview requested:** Show first 5 cards, then offer [m]ore / [s]ave / [c]ancel

---

## OUTPUT FORMAT

### New Deck
```markdown
# [Deck Title]

Source: [filename]
Generated: YYYY-MM-DD

---

Q: First question
A: First answer
   [Citation]

---

C: First cloze with [term] bracketed.

(Citation)

---

[... more cards ...]
```

### Augmenting Existing Deck

Append to end of file:
```markdown
[existing content...]

---

# --- Added YYYY-MM-DD from [source filename] ---

---

Q: New question
A: New answer
   [Citation]

---

[... more new cards ...]
```

---

## CONTENT PRIORITIES

### Primary Content (Generate cards from)
- Main concepts, theories, or arguments from body text
- Definitions of key terms and specialized vocabulary
- Important facts, data, or findings explicitly stated
- Quotes or passages emphasized by the author
- Explicit conclusions, takeaways, or summary points
- Application or practical implications clearly stated

### Secondary Content (Include sparingly)
- Footnotes/endnotes ONLY if referenced in main text
- Supplementary examples ONLY if central to the argument
- Background context ONLY if emphasized as important
- Cross-references ONLY if the text makes the connection

### Exclude (Never generate cards from)
- Supplementary section content not mentioned in main text
- Tangential details in footnotes or appendices
- Citations or references not discussed in body
- Your own inferences or connections
- External knowledge not in the source

---

## ERROR HANDLING

| Situation | Response |
|-----------|----------|
| `pdftotext` not found | "Install poppler-utils: `sudo apt install poppler-utils`" |
| PDF extraction fails | "Could not extract text. Provide a .txt file instead?" |
| Source < 500 words | "Source may be too short. Expect fewer cards. Continue?" |
| No clear structure | "No sections detected. Will generate without section citations." |
| All cards flagged | "Quality concerns with source material. Review carefully or try different source." |
| Deck file not writable | "Cannot write to [path]. Check permissions." |

---

## QUICK REFERENCE

**Invocation:** `/flashcard [path]`

**Card types:**
- `Q:` / `A:` - Question and answer
- `C:` - Cloze deletion (ONE bracket only)

**Citation formats:**
- Q&A: `[Source: "quote" - Section X, page N]`
- Cloze: `(Section X, page N)` - no quotes!

**Flag triggers:**
- Synthesized from multiple sources
- Ambiguous question
- Significant paraphrase
- Supplementary section content
- Citation reveals cloze answer

**Workflow:**
1. Gather requirements (source, target deck)
2. Extract + generate with inline verification
3. Review flagged cards with user
4. Save to deck
5. Log any issues to feedback file

---

## POST-RUN: FEEDBACK LOGGING

After completing a run, check for issues to log:

### Log If Any Of These Occurred
- False positive: Good card flagged unnecessarily
- False negative: Bad card passed verification
- Format issue not caught
- Edge case not handled (PDF structure, encoding, etc.)
- User confusion with prompts
- Unexpected error

### Feedback Format

Append to `.claude/skills/flashcard.feedback.md`:
```markdown
## YYYY-MM-DD: [Brief title]
Issue: [What went wrong]
Context: [Source file, workflow step, card example]
Expected: [What should have happened]
Suggestion: [How to improve the skill]
```

### Example Entries

```markdown
## 2026-01-18: Multi-column PDF extraction garbled
Issue: pdftotext merged columns incorrectly
Context: cards/raw/newsletter.pdf - 2-column layout
Expected: Text should flow naturally
Suggestion: Add -layout flag or warn about multi-column PDFs

## 2026-01-18: False flag on valid paraphrase
Issue: Card flagged as "paraphrased" but meaning was exact
Context: Source "saved by grace" → Card "salvation by grace"
Expected: Should recognize semantic equivalence
Suggestion: Refine paraphrase detection to allow minor word variations
```

### Skip Logging If
- Run completed without issues
- All flags were legitimate
- User made no corrections

### Improvement Trigger

When feedback file reaches 5+ entries OR same issue appears twice:
1. Notify user: "Feedback accumulated. Run `/improve flashcard` to review."
2. Or proactively suggest fixes during next skill invocation
