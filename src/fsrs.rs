use chrono::Duration;

pub const WEIGHTS: [f64; 19] = [
    0.40255, 1.18385, 3.173, 15.69105, 7.1949, 0.5345, 1.4604, 0.0046, 1.54575, 0.1192, 1.01925,
    1.9395, 0.11, 0.29605, 2.2698, 0.2315, 2.9898, 0.51655, 0.6621,
];

const F: f64 = 19.0 / 81.0;
const C: f64 = -0.5;
const TARGET_RECALL: f64 = 0.9;
const MIN_INTERVAL: f64 = 1.0;
const MAX_INTERVAL: f64 = 256.0;
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
pub fn calculate_recall(interval: f64, stability: f64) -> f64 {
    (1.0 + F * (interval / stability)).powf(C)
}

pub fn calulate_interval(recall: f64, stability: f64) -> f64 {
    (stability / F) * (recall.powf(1.0 / C) - 1.0)
}

pub fn initial_stability(review_status: ReviewStatus) -> f64 {
    match review_status {
        ReviewStatus::Fail => WEIGHTS[0],
        ReviewStatus::Pass => WEIGHTS[2],
    }
}

fn calculate_stability(
    difficulty: f64,
    stability: f64,
    recall: f64,
    review_status: ReviewStatus,
) -> f64 {
    if review_status == ReviewStatus::Fail {
        let d_f = difficulty.powf(-WEIGHTS[12]);
        let s_f = (stability + 1.0).powf(WEIGHTS[13]) - 1.0;
        let r_f = f64::exp(WEIGHTS[14] * (1.0 - recall));
        let c_f = WEIGHTS[11];
        let s_f = d_f * s_f * r_f * c_f;
        return f64::min(s_f, stability);
    }
    let t_d = 11.0 - difficulty;
    let t_s = stability.powf(-WEIGHTS[9]);
    let t_r = f64::exp(WEIGHTS[10] * (1.0 - recall)) - 1.0;
    let h = 1.0;
    let b = 1.0;
    let c = f64::exp(WEIGHTS[8]);
    let alpha = 1.0 + t_d * t_s * t_r * h * b * c;
    stability * alpha
}

fn clamp_difficulty(difficulty: f64) -> f64 {
    difficulty.clamp(1.0, 10.0)
}

pub fn initial_difficulty(review_status: ReviewStatus) -> f64 {
    let g: f64 = review_status.score() as f64;
    clamp_difficulty(WEIGHTS[4] - f64::exp(WEIGHTS[5] * (g - 1.0)) + 1.0)
}

pub fn new_difficulty(difficulty: f64, review_status: ReviewStatus) -> f64 {
    clamp_difficulty(
        WEIGHTS[7] * initial_difficulty(ReviewStatus::Pass)
            + (1.0 - WEIGHTS[7]) * dp(difficulty, review_status),
    )
}

fn dp(difficulty: f64, review_status: ReviewStatus) -> f64 {
    difficulty + delta_d(review_status) * ((10.0 - difficulty) / 9.0)
}

fn delta_d(review_status: ReviewStatus) -> f64 {
    let g: f64 = review_status.score() as f64;
    -WEIGHTS[6] * (g - 3.0)
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
    pub fn score(&self) -> usize {
        match self {
            ReviewStatus::Pass => 3,
            ReviewStatus::Fail => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReviewedPerformance {
    pub last_reviewed_at: chrono::DateTime<chrono::Utc>,
    pub stability: f64,
    pub difficulty: f64,
    pub interval_raw: f64,
    pub interval_days: usize,
    pub due_date: chrono::DateTime<chrono::Utc>,
    pub review_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Performance {
    #[default]
    New,
    Reviewed(ReviewedPerformance),
}

pub fn update_performance(
    perf: Performance,
    review_status: ReviewStatus,
    reviewed_at: chrono::DateTime<chrono::Utc>,
) -> ReviewedPerformance {
    let (stability, difficulty, review_count): (f64, f64, usize) = match perf {
        Performance::New => (
            initial_stability(review_status),
            initial_difficulty(review_status),
            0,
        ),
        Performance::Reviewed(ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            review_count,
            ..
        }) => {
            let elapsed_days = reviewed_at
                .signed_duration_since(last_reviewed_at)
                .num_seconds() as f64
                / 86_400.0;
            let recall = calculate_recall(elapsed_days.max(0.0), stability);
            let stability = calculate_stability(difficulty, stability, recall, review_status);
            let difficulty = new_difficulty(difficulty, review_status);
            (stability, difficulty, review_count)
        }
    };
    let interval_raw: f64 = calulate_interval(TARGET_RECALL, stability);
    let interval_rounded: f64 = interval_raw.round();
    let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
    let fsrs_duration = Duration::days(interval_clamped as i64);

    let interval_duration = early_interval_cap(review_count, review_status)
        .map(|cap| fsrs_duration.min(cap))
        .unwrap_or(fsrs_duration);
    let interval_effective_days = interval_duration.num_seconds() as f64 / 86_400.0;

    let interval_raw = interval_effective_days;

    let interval_days: usize = interval_duration.num_days().max(1) as usize;
    let due_date: chrono::DateTime<chrono::Utc> = reviewed_at + interval_duration;
    ReviewedPerformance {
        last_reviewed_at: reviewed_at,
        stability,
        difficulty,
        interval_raw,
        interval_days,
        due_date,
        review_count: review_count + 1,
    }
}

#[cfg(test)]
mod tests {

    use super::{
        MAX_INTERVAL, MIN_INTERVAL, Performance, ReviewStatus, ReviewedPerformance,
        update_performance,
    };

    use chrono::Duration;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-2
    }

    #[test]
    fn test_update_new_card() {
        let reviewed_at = chrono::Utc::now();
        let result = update_performance(Performance::New, ReviewStatus::Pass, reviewed_at);
        let ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date: _,
            review_count,
        } = result;
        assert_eq!(last_reviewed_at, reviewed_at);
        assert!(approx_eq(stability, 3.17));
        assert!(approx_eq(difficulty, 5.28));
        assert!(approx_eq(interval_raw, 0.0006944444444444445));
        assert_eq!(interval_days, 1);
        assert_eq!(review_count, 1);
    }

    #[test]
    fn test_update_already_reviewed_card() {
        let now = chrono::Utc::now();
        let duration = Duration::days(3);
        let last_reviewed_at = now - duration;
        let initial_perf = ReviewedPerformance {
            last_reviewed_at,
            stability: 3.17,
            difficulty: 5.28,
            interval_raw: 3.17,
            interval_days: 3,
            due_date: now + duration,
            review_count: 1,
        };
        let reviewed_at = now;
        let result = update_performance(
            Performance::Reviewed(initial_perf),
            ReviewStatus::Pass,
            reviewed_at,
        );
        let ReviewedPerformance {
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date: _,
            review_count,
        } = result;
        assert_eq!(last_reviewed_at, reviewed_at);
        assert!(approx_eq(stability, 10.739));
        assert!(approx_eq(difficulty, 5.280));
        assert!(approx_eq(interval_raw, 0.006944444444444444));
        assert_eq!(interval_days, 1);
        assert_eq!(review_count, 2);
    }

    #[test]
    fn test_reviews() {
        let mut reviewed_at = chrono::Utc::now();
        let mut performance = update_performance(Performance::New, ReviewStatus::Pass, reviewed_at);
        for _ in 0..100 {
            let interval_raw = performance.interval_raw;
            let interval_rounded: f64 = interval_raw.round();
            let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
            let interval_duration: Duration = Duration::days(interval_clamped as i64);
            reviewed_at += interval_duration;

            performance = update_performance(
                Performance::Reviewed(performance),
                ReviewStatus::Pass,
                reviewed_at,
            );
        }
        assert_eq!(performance.review_count, 101);
        assert_eq!(performance.interval_days, 256);
        assert!(approx_eq(performance.difficulty, 5.28));
        assert!(approx_eq(performance.stability, 25827.806046079717));

        for _ in 0..100 {
            let interval_raw = performance.interval_raw;
            let interval_rounded: f64 = interval_raw.round();
            let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
            let interval_duration: Duration = Duration::days(interval_clamped as i64);
            reviewed_at += interval_duration;

            performance = update_performance(
                Performance::Reviewed(performance),
                ReviewStatus::Fail,
                reviewed_at,
            );
        }
        assert_eq!(performance.review_count, 201);
        assert_eq!(performance.interval_days, 1);
        assert!(approx_eq(performance.difficulty, 9.9337));
        assert!(approx_eq(performance.stability, 0.148424));
    }
}
