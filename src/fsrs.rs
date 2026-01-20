use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use fsrs::{DEFAULT_PARAMETERS, FSRS, MemoryState};

const DESIRED_RETENTION: f32 = 0.9;
const SECONDS_PER_DAY: f64 = 86_400.0;

pub const LEARN_AHEAD_THRESHOLD_MINS: Duration = Duration::minutes(20);

fn early_interval_cap(review_count: usize, review_status: ReviewStatus) -> Option<Duration> {
    match review_count {
        0 => Some(Duration::minutes(1)),
        1 => match review_status {
            ReviewStatus::Pass => Some(Duration::minutes(10)),
            ReviewStatus::Fail => Some(Duration::minutes(1)),
        },
        2 => match review_status {
            ReviewStatus::Pass => Some(Duration::days(1)),
            ReviewStatus::Fail => Some(Duration::minutes(10)),
        },
        _ => None,
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum ReviewStatus {
    Pass,
    Fail,
}

impl ReviewStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ReviewStatus::Pass => "Pass",
            ReviewStatus::Fail => "Fail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewedPerformance {
    pub last_reviewed_at: DateTime<Utc>,
    pub stability: f64,
    pub difficulty: f64,
    pub interval_raw: f64,
    pub interval_days: usize,
    pub due_date: DateTime<Utc>,
    pub review_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Performance {
    #[default]
    New,
    Reviewed(ReviewedPerformance),
}

fn fsrs_model() -> Result<FSRS> {
    FSRS::new(Some(&DEFAULT_PARAMETERS)).context("failed to initialize FSRS model")
}

fn next_state_for_review(
    next_states: fsrs::NextStates,
    review_status: ReviewStatus,
) -> fsrs::ItemState {
    match review_status {
        ReviewStatus::Pass => next_states.good,
        ReviewStatus::Fail => next_states.again,
    }
}

pub fn update_performance(
    perf: Performance,
    review_status: ReviewStatus,
    reviewed_at: DateTime<Utc>,
) -> Result<ReviewedPerformance> {
    let (memory_state, last_reviewed_at, review_count) = match perf {
        Performance::New => (None, None, 0),
        Performance::Reviewed(ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            review_count,
            ..
        }) => (
            Some(MemoryState {
                stability: stability as f32,
                difficulty: difficulty as f32,
            }),
            Some(last_reviewed_at),
            review_count,
        ),
    };

    let elapsed_days = last_reviewed_at
        .map(|last| reviewed_at.signed_duration_since(last).num_days().max(0) as u32)
        .unwrap_or(0);

    let fsrs = fsrs_model()?;
    let next_states = fsrs.next_states(memory_state, DESIRED_RETENTION, elapsed_days)?;
    let next_state = next_state_for_review(next_states, review_status);

    let interval_raw = next_state.interval as f64;
    let fsrs_seconds = (interval_raw * SECONDS_PER_DAY).round().max(1.0) as i64;
    let fsrs_duration = Duration::seconds(fsrs_seconds);

    let interval_duration = early_interval_cap(review_count, review_status)
        .map(|cap| fsrs_duration.min(cap))
        .unwrap_or(fsrs_duration);

    let interval_effective_days = interval_duration.num_seconds() as f64 / SECONDS_PER_DAY;
    let interval_days = interval_duration.num_days().max(0) as usize;
    let due_date = reviewed_at + interval_duration;

    Ok(ReviewedPerformance {
        last_reviewed_at: reviewed_at,
        stability: next_state.memory.stability as f64,
        difficulty: next_state.memory.difficulty as f64,
        interval_raw: interval_effective_days,
        interval_days,
        due_date,
        review_count: review_count + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::{Performance, ReviewStatus, ReviewedPerformance, update_performance};
    use chrono::Duration;

    #[test]
    fn test_update_new_card() {
        let reviewed_at = chrono::Utc::now();
        let result = update_performance(Performance::New, ReviewStatus::Pass, reviewed_at).unwrap();
        assert_eq!(result.last_reviewed_at, reviewed_at);
        assert!(result.stability.is_finite());
        assert!(result.difficulty.is_finite());
        assert!(result.interval_days == 0);
        assert_eq!(result.interval_raw, 0.0006944444444444445);
        assert_eq!(result.review_count, 1);
    }

    #[test]
    fn test_short_term_learning() {
        let now = chrono::Utc::now();
        let duration = Duration::days(3);
        let last_reviewed_at = now - duration;
        let initial_perf = ReviewedPerformance {
            last_reviewed_at,
            stability: 3.0,
            difficulty: 5.0,
            interval_raw: 3.0,
            interval_days: 3,
            due_date: now,
            review_count: 1,
        };
        let result =
            update_performance(Performance::Reviewed(initial_perf), ReviewStatus::Pass, now)
                .unwrap();
        assert_eq!(result.last_reviewed_at, now);
        assert!(result.interval_days == 0);
        assert_eq!(result.review_count, 2);
    }

    #[test]
    fn test_update_failed_review() {
        let now = chrono::Utc::now();
        let initial_perf = ReviewedPerformance {
            last_reviewed_at: now - Duration::days(4),
            stability: 3.0,
            difficulty: 5.0,
            interval_raw: 4.0,
            interval_days: 4,
            due_date: now + Duration::days(4),
            review_count: 3,
        };
        let result =
            update_performance(Performance::Reviewed(initial_perf), ReviewStatus::Fail, now)
                .unwrap();
        assert_eq!(result.interval_raw, 0.7213425925925926);
        assert_eq!(result.review_count, 4);
    }
}
