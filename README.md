# repeater

<p align="center">
  <a href="https://github.com/shaankhosla/repeater/actions/workflows/ci.yaml">
    <img alt="CI Status" src="https://img.shields.io/github/actions/workflow/status/shaankhosla/repeater/ci.yaml?branch=main&label=CI&logo=github" />
  </a>
  <a href="https://shaankhosla.github.io/repeater/">
    <img alt="Documentation" src="https://img.shields.io/badge/docs-GitHub%20Pages-blue?logo=github" />
  </a>
  <a href="https://github.com/shaankhosla/repeater/releases">
    <img alt="Latest Release" src="https://img.shields.io/github/v/release/shaankhosla/repeater?display_name=tag&sort=semver&logo=github" />
  </a>
  <a href="LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/shaankhosla/repeater?color=informational" />
  </a>
</p>

`repeater` is a command-line flashcard program that uses [spaced repetition](https://en.wikipedia.org/wiki/Spaced_repetition) to boost your memory retention. It’s like a lightweight, text-based Anki you run in your terminal. Your decks are kept in Markdown, progress is tracked in SQLite, and reviews are scheduled with Free Spaced Repetition Scheduler (FSRS), a state-of-the-art algorithm targeting 90% recall.


<img src="check_example.png" alt="Checking card progress" />

> [!NOTE]
> You can find the main documentation, including installation guides, at [https://shaankhosla.github.io/repeater/](https://shaankhosla.github.io/repeater/).

## Features

- Cards live in `.md` files, so edit them using your favorite markdown editor, back them up with version control, and let them live alongside regular notes.
 - Progress is tracked with a hash of the meaningful words, so real text changes reset progress while punctuation or casing tweaks do not.
- Free Spaced Repetition Scheduler (FSRS), a state-of-the-art algorithm targeting 90% recall, automatically schedules reviews for you.
- Terminal UX: `repeater drill` renders cards with ratatui; `repeater create` launches an editor dedicated to card capture; `repeater check` displays progress at a glance.
- Inline media support: reference local images/audio/video inside your decks and open them from a drill session without leaving the terminal.
- Import from Anki: convert `.apkg` exports into Markdown decks with `repeater import` so you can bring your existing collection along.


## Installation

### Install script (Linux & macOS) - Recommended

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/shaankhosla/repeater/releases/latest/download/repeater-installer.sh | sh
```

### Homebrew (macOS)

```
brew install shaankhosla/tap/repeater
```

### Windows (PowerShell)

```
irm https://github.com/shaankhosla/repeater/releases/latest/download/repeater-installer.ps1 | iex
```

### npm 

```
npm install @shaankhosla/repeater
```

## Quick Start

1. Create a deck in Markdown (`cards/neuro.md`):

   ```markdown
   You can put your normal notes here, `repeater` will ignore them.
   Once a "Q:,A:,C:" block is detected, it will automatically
   turn it into a card.

   Q: What does a synaptic vesicle store?
   A: Neurotransmitters awaiting release.

   ---
   Use a separator to mark the end of a card^
   Then feel free to go back to adding regular notes.

   C: Speech is [produced] in [Broca's] area.
   ```


2. Index the cards and start a session:

   ```
   repeater drill cards
   ```

   - `Space`/`Enter`: reveal the answer or cloze.
   - `O`: open the first media file (image/audio/video) referenced in the current card before revealing the answer.
   - `1`: mark as `Fail`, `2`: mark as `Pass`.
   - `Esc` or `Ctrl+C`: end the session early (progress so far is saved).
