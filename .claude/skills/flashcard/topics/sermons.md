# Topic Specialty: Sermons

This document provides sermon-specific guidance for the `/flashcard` skill. The core skill is topic-agnostic; this file adapts it for sermon content.

**When to use:** Processing sermon PDFs, talks, or religious teaching materials.

---

## Sermon Document Structure

### Typical PDF Structure

```
Sermon Title + Scripture Reference
├── Introduction
├── Main Points (1, 2, 3 or I, II, III)
│   └── Sub-points (A, B, C or a, b, c)
├── Application/Conclusion
└── Research Notes [SECONDARY - use sparingly]
```

### Example Mental Map

When parsing a sermon PDF, build a structure like this:

```
Document: "Ephesians 2:1-10 - Dead in Sin, Alive in Christ"
├── Introduction (page 1)
├── Section 1: Dead in Sin (pages 2-5)
│   ├── 1.A: The condition described
│   └── 1.B: The cause explained
├── Section 2: Alive in Christ (pages 6-10)
│   ├── 2.A: But God...
│   └── 2.B: By grace through faith
├── Section 3: Conclusion (pages 11-12)
└── Research Notes (pages 13-15) [SECONDARY]
```

---

## Content Priorities for Sermons

### Primary Content (Generate cards from)
- Main teaching points from sermon body
- Key theological concepts explained
- Scripture verses quoted and explained
- Definitions of Greek/Hebrew terms (if explained in sermon)
- Memorable quotes or phrases emphasized by speaker
- Application points clearly stated

### Secondary Content (Include sparingly)
- Research notes ONLY if directly referenced in sermon
- Historical context ONLY if sermon emphasizes it
- Cross-references ONLY if sermon makes the connection

### Exclude (Never generate cards from)
- Research section content not mentioned in sermon
- Tangential historical details
- Commentary citations not discussed in teaching
- Your own inferences or connections
- External knowledge not in the source

---

## Greek/Hebrew Terms

When a sermon explains original language terms:

**Good cloze target:** The Greek/Hebrew word itself
- Include transliteration and meaning
- Bracket the foreign word, not the English translation

**Example:**
```markdown
C: Paul uses the Greek word [nekros] meaning "dead" to describe our spiritual state before Christ.

(Section 1.A, page 3)
```

**Another example:**
```markdown
C: The Hebrew word [hesed] describes God's steadfast, covenant love.

(Section 2.B, page 7)
```

---

## Citation Formats for Sermons

### Q&A Cards
```markdown
Q: Question from sermon?
A: Answer from sermon.
   [Sermon: "exact quote from source" - Section X.Y, page N]
```

**Section reference formats:**
- `Section 2.A` or `Point II.B`
- `Section 1`, `Point 3`
- Whatever matches the sermon's outline structure

**Example:**
```markdown
Q: According to Ephesians 2:8-9, what is the basis of our salvation?
A: Grace through faith, not works - it is the gift of God so that no one may boast.
   [Sermon: "For by grace you have been saved through faith. And this is not your own doing; it is the gift of God, not a result of works, so that no one may boast." - Section 2.A, page 6]
```

### Cloze Cards
```markdown
C: Statement with [key term] bracketed.

(Section X.Y, page N)
```

**Important:** Cloze citations should NOT include quotes (they might reveal the answer).

**Example:**
```markdown
C: The phrase "[But God]" in Ephesians 2:4 marks the turning point from death to life.

(Section 2.A, page 6)
```

---

## Research Section Handling

Sermon PDFs often have a "Research" or "Notes" section at the end. Handle these carefully:

### Flagging Rule: Research Section Content

Flag a card if it derives from research/notes section, not primary teaching.

**Example:**
```
Historical fact from research notes not mentioned in sermon
Action: Flag with note "From research section - confirm inclusion"
```

### When to Include Research Content

Only create cards from research sections if:
1. The sermon explicitly references the information
2. The speaker emphasizes the historical/background detail
3. It's necessary to understand a main teaching point

**Example of good research usage:**
If the sermon says "In first-century Ephesus, the temple of Artemis dominated the skyline" and then the research section provides details about the temple, you can use those details.

**Example of bad research usage:**
If the research section mentions a historical detail about Roman currency, but the sermon never discusses it, skip it.

---

## Special Considerations

### Scripture References
When the sermon quotes or explains a Bible verse:
- The verse itself is part of primary content
- Create cards testing understanding of the verse
- Include the verse reference in your card

**Example:**
```markdown
Q: What does Ephesians 2:1 say about our condition before Christ?
A: We were dead in trespasses and sins.
   [Sermon: "You were dead in the trespasses and sins" - Section 1.A, page 2]
```

### Application Points
Sermons often have "application" sections. These are primary content:
- Create cards testing what believers should do/think/feel
- Keep them concrete, not vague

**Good:**
```markdown
Q: According to this sermon, how should remembering God's grace affect our worship?
A: When we remember who God is and what He has done, worship overflows and our hearts warm toward God.
   [Sermon: "When we remember who God is and what He has done for us and who we are rekindles our hope and restores our worship." - Section 2.B, page 5]
```

**Bad (too vague):**
```markdown
Q: What should we do?
A: Worship God.
```

### Theological Terms
Sermons often use theological vocabulary:
- Good cloze targets: election, predestination, justification, sanctification
- Include the definition if the sermon provides one
- Don't assume theological knowledge—use the sermon's explanation

**Example:**
```markdown
Q: How does this sermon define election?
A: God's eternal, gracious choice to save sinners in Christ—not because of foreseen merit or faith, but according to His sovereign good pleasure.
   [Sermon: "Election is God's eternal, gracious choice to save sinners in Christ—not because of foreseen merit or faith, but according to His sovereign good pleasure" - Section 1.A, page 1]
```

---

## Quick Reference

**Content priority:**
- Primary: Sermon teaching (first ~85% of document)
- Secondary: Research notes (only if referenced in sermon)
- Exclude: Research facts not mentioned in teaching

**Citation format:**
- Q&A: `[Sermon: "quote" - Section X, page N]`
- Cloze: `(Section X, page N)` - no quotes!

**Good cloze targets:**
- Theological terms (election, grace, redemption)
- Greek/Hebrew words (nekros, hesed, agape)
- Key phrases ("But God", "in Christ")
- Scripture references

**Always verify:**
- Every card traces to primary sermon content
- Quotes are exact (word-for-word)
- Section references are accurate
- Research content was mentioned in the sermon
