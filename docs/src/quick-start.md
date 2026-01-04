# Quick Start

1. **Create a deck in Markdown (`cards/neuro.md`).**

   ```markdown
   You can put your normal notes here, `repeat` will ignore them.
   Once a "Q:,A:,C:" block is detected, it will automatically
   turn it into a card.

   Q: What does a synaptic vesicle store?
   A: Neurotransmitters awaiting release.

   ---
   Use a separator to mark the end of a card^
   Then feel free to go back to adding regular notes.

   C: Speech is [produced] in [Broca's] area.
   ```

   Alternatively, launch the built-in editor with:

   ```sh
   repeat create cards/neuro.md
   ```

2. **Index the cards and start a drill session.**

   ```sh
   repeat drill cards
   ```

   - `Space`/`Enter`: reveal the answer or cloze.
   - `O`: open the first media file (image/audio/video) referenced in the current card before revealing the answer.
   - `1`: mark as `Fail`, `2`: mark as `Pass`.
   - `Esc` or `Ctrl+C`: end the session early (progress so far is saved).

3. **Check your collection status.**

   ```sh
   repeat check cards
   ```

   The command prints totals for new/reviewed cards, due/overdue counts, and upcoming workload.
