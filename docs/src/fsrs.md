# FSRS Scheduling

`repeater` schedules every review with the [Free Spaced Repetition Scheduler](https://github.com/open-spaced-repetition/fsrs4anki/wiki/Free-Spaced-Repetition-Scheduler) (FSRS). The upstream model treats each card's *stability* (how long it can be recalled) and *difficulty* (how hard the content feels) as latent variables and adjusts them after every answer. This page documents how `repeater` applies the model, along with the opinionated choices that make it feel lightweight in a terminal workflow.

## Core Parameters

- **Target recall** — Intervals are solved for a 90 % recall probability (`TARGET_RECALL = 0.9`), which is the default in FSRS research and keeps workloads manageable.
- **Weights** — The 19 FSRS-4 weights (`WEIGHTS`) are compiled into the binary instead of being trained per-user. Everyone starts from the same curve, so reviews are predictable even without a calibration phase.
- **State tracked per card** — Each row in `cards.db` stores `stability`, `difficulty`, `interval_raw`, `interval_days`, `due_date`, and `review_count`. The timers you see during drills are derived from these values, while the Markdown deck stays untouched.

## Simplified Feedback Model

Classic FSRS expects four answer buttons. `repeater` distills that into two hotkeys: `Pass` and `Fail`. Internally those map to quality scores of 3 and 1 respectively, so the formulas for `initial_stability`, `initial_difficulty`, `delta_d`, and `calculate_stability` still behave correctly. The benefit is a fast keyboard workflow; the trade-off is that you can't express nuances like "Hard" or "Easy", so the algorithm falls back to its conservative defaults when scheduling recoveries after a lapse.

## Early Review Ramp

FSRS is designed for day-scale intervals, so the code layers a short-term trainer on top:

| Review count before the answer | Result | Max delay |
| --- | --- | --- |
| 0 (brand new) | Pass/Fail | 1 minute |
| 1 | Pass | 10 minutes |
| 1 | Fail | 1 minute |
| 2 | Pass | 1 day |
| 2 | Fail | 10 minutes |

These caps override the usual interval just for the first few answers, which keeps new material in front of you until you can reliably recall it. Once the review count exceeds two, the pure FSRS interval is used.

## Learn-Ahead Window & Queueing

- The spaced repetition queue treats anything due within the next 20 minutes as "due now". This is the `LEARN_AHEAD_THRESHOLD_MINS`, and it means that when you sit down for a session you see cards that are about to become due so you don't have to reopen the app later in the day.
- During a drill, the interval returned from FSRS is compared against the same threshold. If it's shorter than 20 minutes (for example right after a lapse) the card is immediately re-queued in the current session instead of waiting for a later run.
- The daily queue pulls overdue cards first, then cards due later today, and only then does it sprinkle in new cards—subject to your optional daily limits. That ordering makes sure FSRS's promises ("you'll keep 90 % recall") remain accurate even if you have a backlog.

## What Happens After Each Answer

1. The elapsed time since the last review is measured to compute the recall probability FSRS expected at the moment you answered.
2. Depending on whether you pressed `Pass` or `Fail`, the algorithm updates stability and difficulty with the upstream formulas.
3. A new interval is solved for 90 % recall, rounded, clamped, and—if applicable—shortened by the early-review caps above.
4. Metadata in `cards.db` is updated atomically so stats, the `check` command, and future sessions all agree on the next due date.

## Further Reading

- [FSRS whitepaper & wiki](https://github.com/open-spaced-repetition/fsrs4anki/wiki/Free-Spaced-Repetition-Scheduler) — background on the equations `repeater` calls into.
- [FSRS weights repository](https://github.com/open-spaced-repetition/fsrs4anki) — reference implementation and tuning scripts if you want to experiment with your own parameter set.
