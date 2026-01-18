# Repeater Project - Claude Code Guidelines

## Available Skills

| Skill | Purpose | Location |
|-------|---------|----------|
| `/flashcard` | PDF → flashcard generation with verification | `.claude/skills/flashcard.md` |
| `/improve` | Process feedback and update skills | `.claude/skills/improve.md` |
| `/audit` | Quality assessment of code, docs, plans | `~/.claude/skills/audit` |

---

## Flashcard Creation

**Primary method:** Use the `/flashcard` skill for PDF → flashcard generation.

```
/flashcard                      # Interactive mode
/flashcard cards/raw/topic.pdf  # With source file
```

The skill handles: PDF extraction, card generation, verification, duplicate detection, and user review.

**Skill location:** `.claude/skills/flashcard.md`

### Manual Workflow (Fallback)

If the skill is unavailable or for quick edits:

1. Extract: `pdftotext /path/to/file.pdf /tmp/output.txt`
2. Create cards following format below
3. Verify every card against source (see full checklist in `.claude/skills/flashcard.md`)
4. Save to `cards/<deck>.md`

### Card Format Quick Reference

```markdown
Q: Question from source?
A: Answer from source.
   [Source: "quote" - Section X, page N]

---

C: Statement with [key term] bracketed.

(Section X, page N)
```

**Rules:**
- Q:/A:/C: at column 0
- ONE bracket per cloze card
- Cloze citations: NO quotes (reveals answer)
- Every card must trace to source

## File Organization

```
cards/
├── raw/           # Source PDFs organized by topic
│   └── <topic>/
│       └── *.pdf
└── <deck>.md      # Flashcard decks
```

## Quality Standards

1. **No hallucination** - Every fact traces to source
2. **Direct quotes preferred** - Paraphrase only when necessary
3. **Section references required** - User can find source
4. **Verify before delivery** - Re-read source to confirm

---

## Skill Feedback Loop

Skills improve through usage. When issues occur, capture them for future refinement.

### During Skill Execution

When encountering issues with `/flashcard` (or any skill):

**Log automatically** - After each run, if any of these occur:
- Card flagged that shouldn't have been (false positive)
- Card passed that should have been flagged (false negative)
- Format issue not caught by verification
- Edge case not handled
- Confusing prompt or unclear instruction

Append to `.claude/skills/flashcard.feedback.md`:
```markdown
## YYYY-MM-DD: [Brief title]
Issue: [What went wrong]
Context: [Source file, card content, or workflow step]
Expected: [What should have happened]
Suggestion: [How to fix the skill]
```

### Improvement Triggers

Review and update skill when:
- Feedback file has 5+ entries
- Same issue reported twice
- User explicitly requests `/improve flashcard`

See `.claude/skills/improve.md` for the full improvement workflow.

### Improvement Process

1. Read `.claude/skills/flashcard.feedback.md`
2. Identify patterns (repeated issues, common edge cases)
3. Update `.claude/skills/flashcard.md` with fixes
4. Archive resolved feedback entries
5. Test with next run

### Feedback Categories

| Category | Example | Fix Location |
|----------|---------|--------------|
| False flag | Good card flagged as "ambiguous" | Flagging Rules section |
| Missed issue | Bad card passed verification | Verification Checklist |
| Format bug | Wrong citation format generated | Card Format section |
| UX confusion | User didn't understand prompt | Phase 1/3 prompts |
| Edge case | Multi-column PDF broke extraction | Error Handling |

---

## Topic-Specific Guidance

The flashcard skill is topic-agnostic and adapts to various content types:
- **Academic**: Textbook chapters, research papers
- **Technical**: Documentation, specifications
- **Educational**: Lecture notes, course materials
- **Religious**: Sermons, Bible studies, theological works

**Domain-specific patterns:** See `.claude/skills/flashcard/topics/`

### Available Topic Guides

- **sermons.md** - Sermon-specific document structure, content priorities, Greek/Hebrew terms, citation formats

**Adding new topics:** Create a new `.md` file in `.claude/skills/flashcard/topics/` following the pattern in `sermons.md`.
