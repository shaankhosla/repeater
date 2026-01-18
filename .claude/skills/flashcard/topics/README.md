# Flashcard Topic Specialties

This directory contains domain-specific guidance for the `/flashcard` skill.

## Purpose

The core flashcard skill (in `../SKILL.md`) is topic-agnostic and works for any structured document. This directory provides specialized patterns for different content types.

## Available Topics

- **sermons.md** - Religious sermons, talks, and Bible studies
  - Sermon document structure
  - Content priorities (teaching vs research sections)
  - Greek/Hebrew term handling
  - Citation formats

## Adding New Topics

To add guidance for a new domain:

1. Create `<topic>.md` in this directory
2. Follow the structure in `sermons.md`:
   - **Document Structure** - Typical outline/organization
   - **Content Priorities** - What to emphasize vs skip
   - **Special Considerations** - Domain-specific vocabulary, citation formats, etc.
   - **Quick Reference** - Summary of key patterns
3. Update the "Available Topic Guides" section in `CLAUDE.md`

## Examples of Future Topics

- **textbooks.md** - Academic textbook chapters
- **research-papers.md** - Journal articles, conference papers
- **documentation.md** - Technical docs, API references
- **lectures.md** - Lecture notes, course materials

## Using Topic Guides

When processing a document, the AI will:
1. Use the core skill for general workflow (extraction, verification, duplicate detection)
2. Consult the relevant topic guide for domain-specific patterns
3. Adapt citation formats, content priorities, and examples to match the domain
