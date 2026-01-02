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
pub const MINUTES_PER_DAY: f64 = 60.0 * 24.0;
const LEARNING_A_INTERVAL_MINS: i64 = 1;
const LEARNING_B_INTERVAL_MINS: i64 = 10;

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
pub struct FsrsStats {
    pub stability: f64,
    pub difficulty: f64,
    pub scheduling_stats: SchedulingStats,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SchedulingStats {
    pub last_reviewed_at: chrono::DateTime<chrono::Utc>,
    pub review_count: usize,
    pub interval_raw: f64,
    pub interval_days: usize,
    pub due_date: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
pub enum ReviewStage {
    #[default]
    New,
    LearningA,
    LearningB,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Performance {
    #[default]
    New,
    LearningA(SchedulingStats),
    LearningB(SchedulingStats),
    Review(FsrsStats),
}
impl Performance {
    pub fn stage(&self) -> ReviewStage {
        match self {
            Performance::New => ReviewStage::New,
            Performance::LearningA(_) => ReviewStage::LearningA,
            Performance::LearningB(_) => ReviewStage::LearningB,
            Performance::Review(_) => ReviewStage::Review,
        }
    }
}

fn learning_stats(
    reviewed_at: chrono::DateTime<chrono::Utc>,
    review_count: usize,
    interval_mins: i64,
) -> SchedulingStats {
    SchedulingStats {
        last_reviewed_at: reviewed_at,
        review_count,
        interval_raw: interval_mins as f64 / MINUTES_PER_DAY,
        interval_days: 0,
        due_date: reviewed_at + Duration::minutes(interval_mins),
    }
}

fn next_review_count(old: Option<&SchedulingStats>) -> usize {
    old.map(|s| s.review_count + 1).unwrap_or(1)
}

pub fn update_performance(
    perf: Performance,
    review_status: ReviewStatus,
    reviewed_at: chrono::DateTime<chrono::Utc>,
) -> Performance {
    use Performance::*;
    use ReviewStatus::*;

    match (perf, review_status) {
        // ── New ──────────────────────────────────────────────
        (New, Pass) => LearningB(learning_stats(reviewed_at, 1, LEARNING_B_INTERVAL_MINS)),
        (New, Fail) => LearningA(learning_stats(reviewed_at, 1, LEARNING_A_INTERVAL_MINS)),

        // ── Learning A ───────────────────────────────────────
        (LearningA(old), Fail) => LearningA(learning_stats(
            reviewed_at,
            next_review_count(Some(&old)),
            LEARNING_A_INTERVAL_MINS,
        )),
        (LearningA(old), Pass) => LearningB(learning_stats(
            reviewed_at,
            next_review_count(Some(&old)),
            LEARNING_B_INTERVAL_MINS,
        )),

        // ── Learning B ───────────────────────────────────────
        (LearningB(old), Fail) => LearningA(learning_stats(
            reviewed_at,
            next_review_count(Some(&old)),
            LEARNING_A_INTERVAL_MINS,
        )),
        (perf @ LearningB(_), Pass) => Review(fsrs_schedule(perf, Pass, reviewed_at)),

        // ── Review ───────────────────────────────────────────
        (Review(old_fsrs), Fail) => LearningB(learning_stats(
            reviewed_at,
            next_review_count(Some(&old_fsrs.scheduling_stats)),
            LEARNING_B_INTERVAL_MINS,
        )),
        (perf @ Review(_), Pass) => Review(fsrs_schedule(perf, Pass, reviewed_at)),
    }
}
pub fn fsrs_schedule(
    perf: Performance,
    review_status: ReviewStatus,
    reviewed_at: chrono::DateTime<chrono::Utc>,
) -> FsrsStats {
    let (stability, difficulty, prev_review_count): (f64, f64, usize) = match perf {
        Performance::New | Performance::LearningA(_) | Performance::LearningB(_) => (
            initial_stability(review_status),
            initial_difficulty(review_status),
            0,
        ),
        Performance::Review(rp) => {
            let elapsed_days = reviewed_at
                .signed_duration_since(rp.scheduling_stats.last_reviewed_at)
                .num_seconds() as f64
                / 86_400.0;

            let recall = calculate_recall(elapsed_days.max(0.0), rp.stability);
            let stability = calculate_stability(rp.difficulty, rp.stability, recall, review_status);
            let difficulty = new_difficulty(rp.difficulty, review_status);

            (stability, difficulty, rp.scheduling_stats.review_count)
        }
    };
    let interval_raw: f64 = calulate_interval(TARGET_RECALL, stability);
    let interval_rounded: f64 = interval_raw.round();
    let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
    let interval_days: usize = interval_clamped as usize;
    let interval_duration: Duration = Duration::days(interval_clamped as i64);
    let due_date: chrono::DateTime<chrono::Utc> = reviewed_at + interval_duration;
    let scheduling_stats = SchedulingStats {
        last_reviewed_at: reviewed_at,
        review_count: prev_review_count + 1,
        interval_raw,
        interval_days,
        due_date,
    };
    FsrsStats {
        stability,
        difficulty,
        scheduling_stats,
    }
}

#[cfg(test)]
mod tests {

    use super::{
        FsrsStats, LEARNING_A_INTERVAL_MINS, LEARNING_B_INTERVAL_MINS, MAX_INTERVAL, MIN_INTERVAL,
        Performance, ReviewStatus, SchedulingStats, fsrs_schedule, update_performance,
    };

    use chrono::Duration;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-2
    }

    #[test]
    fn test_update_new_card() {
        let reviewed_at = chrono::Utc::now();
        let result = fsrs_schedule(Performance::New, ReviewStatus::Pass, reviewed_at);

        let last_reviewed_at = result.scheduling_stats.last_reviewed_at;
        let review_count = result.scheduling_stats.review_count;
        let interval_raw = result.scheduling_stats.interval_raw;
        let interval_days = result.scheduling_stats.interval_days;
        let stability = result.stability;
        let difficulty = result.difficulty;

        assert_eq!(last_reviewed_at, reviewed_at);
        assert!(approx_eq(stability, 3.17));
        assert!(approx_eq(difficulty, 5.28));
        assert!(approx_eq(interval_raw, 3.17));
        assert_eq!(interval_days, 3);
        assert_eq!(review_count, 1);
    }

    #[test]
    fn test_update_already_reviewed_card() {
        let now = chrono::Utc::now();
        let duration = Duration::days(3);
        let last_reviewed_at = now - duration;
        let scheduling_stats = SchedulingStats {
            last_reviewed_at,
            review_count: 1,
            interval_raw: 3.17,
            interval_days: 3,
            due_date: now + duration,
        };
        let initial_perf = FsrsStats {
            stability: 3.17,
            difficulty: 5.28,
            scheduling_stats,
        };
        let reviewed_at = now;
        let result = fsrs_schedule(
            Performance::Review(initial_perf),
            ReviewStatus::Pass,
            reviewed_at,
        );
        let last_reviewed_at = result.scheduling_stats.last_reviewed_at;
        let review_count = result.scheduling_stats.review_count;
        let interval_raw = result.scheduling_stats.interval_raw;
        let interval_days = result.scheduling_stats.interval_days;
        let stability = result.stability;
        let difficulty = result.difficulty;
        assert_eq!(last_reviewed_at, reviewed_at);
        assert!(approx_eq(stability, 10.739));
        assert!(approx_eq(difficulty, 5.280));
        assert!(approx_eq(interval_raw, 10.739));
        assert_eq!(interval_days, 11);
        assert_eq!(review_count, 2);
    }

    #[test]
    fn test_reviews() {
        let mut reviewed_at = chrono::Utc::now();
        let mut performance = fsrs_schedule(Performance::New, ReviewStatus::Pass, reviewed_at);
        for _ in 0..100 {
            let interval_raw = performance.scheduling_stats.interval_raw;
            let interval_rounded: f64 = interval_raw.round();
            let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
            let interval_duration: Duration = Duration::days(interval_clamped as i64);
            reviewed_at += interval_duration;

            performance = fsrs_schedule(
                Performance::Review(performance),
                ReviewStatus::Pass,
                reviewed_at,
            );
        }
        assert_eq!(performance.scheduling_stats.review_count, 101);
        assert_eq!(performance.scheduling_stats.interval_days, 256);
        assert!(approx_eq(performance.difficulty, 5.28));
        assert!(approx_eq(performance.stability, 26315.03905930558));

        for _ in 0..100 {
            let interval_raw = performance.scheduling_stats.interval_raw;
            let interval_rounded: f64 = interval_raw.round();
            let interval_clamped: f64 = interval_rounded.clamp(MIN_INTERVAL, MAX_INTERVAL);
            let interval_duration: Duration = Duration::days(interval_clamped as i64);
            reviewed_at += interval_duration;

            performance = fsrs_schedule(
                Performance::Review(performance),
                ReviewStatus::Fail,
                reviewed_at,
            );
        }
        assert_eq!(performance.scheduling_stats.review_count, 201);
        assert_eq!(performance.scheduling_stats.interval_days, 1);
        assert!(approx_eq(performance.difficulty, 9.9337));
        assert!(approx_eq(performance.stability, 0.148424));
    }

    #[test]
    fn learning_a_interval_matches_one_minute() {
        let reviewed_at = chrono::Utc::now();
        let performance = update_performance(Performance::New, ReviewStatus::Fail, reviewed_at);
        let Performance::LearningA(reviewed) = performance else {
            panic!("expected learning A stage");
        };
        assert_eq!(
            reviewed.due_date,
            reviewed_at + Duration::minutes(LEARNING_A_INTERVAL_MINS)
        );
        assert_eq!(reviewed.interval_days, 0);
        assert!(approx_eq(
            reviewed.interval_raw,
            LEARNING_A_INTERVAL_MINS as f64 / (60.0 * 24.0)
        ));
    }

    #[test]
    fn learning_b_interval_matches_ten_minutes() {
        let reviewed_at = chrono::Utc::now();
        let performance = update_performance(Performance::New, ReviewStatus::Pass, reviewed_at);
        let Performance::LearningB(reviewed) = performance else {
            panic!("expected learning B stage");
        };
        assert_eq!(
            reviewed.due_date,
            reviewed_at + Duration::minutes(LEARNING_B_INTERVAL_MINS)
        );
        assert_eq!(reviewed.interval_days, 0);
        assert!(approx_eq(
            reviewed.interval_raw,
            LEARNING_B_INTERVAL_MINS as f64 / (60.0 * 24.0)
        ));
    }
}
