# repeat

<p align="center">
  <a href="https://github.com/shaankhosla/repeat/actions/workflows/ci.yaml">
    <img alt="CI Status" src="https://img.shields.io/github/actions/workflow/status/shaankhosla/repeat/ci.yaml?branch=main&label=CI&logo=github" />
  </a>
  <a href="https://github.com/shaankhosla/repeat/releases">
    <img alt="Latest Release" src="https://img.shields.io/github/v/release/shaankhosla/repeat?display_name=tag&sort=semver&logo=github" />
  </a>
  <a href="LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/shaankhosla/repeat?color=informational" />
  </a>
</p>

`repeat` is a command-line flashcard program that uses [spaced repetition](https://en.wikipedia.org/wiki/Spaced_repetition) to boost your memory retention. It’s like a lightweight, text-based Anki you run in your terminal. Your decks are kept in Markdown, progress is tracked in SQLite, and reviews are scheduled with Free Spaced Repetition Scheduler (FSRS), a state-of-the-art algorithm targeting 90% recall.

<p align="center">
  <img src="create_example.png" alt="Creating cards in the built-in editor" width="45%" />
  <img src="check_example.png" alt="Checking card progress" width="45%" />
</p>

## Features

- Cards live in `.md` files, so edit them using your favorite markdown editor, back them up with version control, and let them live alongside regular notes.
- Progress is tracked with a hash of the card content, so edits automatically reset their progress.
- Free Spaced Repetition Scheduler (FSRS), a state-of-the-art algorithm targeting 90% recall, automatically schedules reviews for you.
- Terminal UX: `repeat drill` renders cards with ratatui; `repeat create` launches an editor dedicated to card capture; `repeat check` displays progress at a glance.
- Import from Anki: convert `.apkg` exports into Markdown decks with `repeat import` so you can bring your existing collection along.

## Documentation

Installation, quick-start, and usage guides now live in the [mdBook documentation](https://shaankhosla.github.io/repeat/). You can also build them locally with `mdbook build docs`.

## Development

Run the lint/test suite with:

```
cargo fmt --all
cargo clippy
cargo test
```

The repository also ships a `just precommit` recipe that runs the same checks.


## Roadmap

- [X] Import from Anki
- [ ] Allow scrolling to other cards in a collection while creating a new card
- [ ] Edit an existing card while keeping the progress intact
- [ ] Allow for a fuzzy search of existing cards
- [ ] Use LLMs to import from various content sources


## License

Licensed under the Apache License, Version 2.0. See `LICENSE` for details.
