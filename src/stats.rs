use std::collections::{BTreeMap, HashMap};

use std::path::PathBuf;

use crate::card::Card;
use crate::crud::CardStatsRow;
use crate::fsrs::{ReviewStage, calculate_recall};

#[derive(Debug, Default)]
pub struct CardStats {
    pub total_cards_in_db: i64,
    pub num_cards: i64,
    pub card_lifecycles: HashMap<CardLifeCycle, i64>,
    pub due_cards: i64,
    pub upcoming_week: BTreeMap<String, usize>,
    pub upcoming_month: i64,
    pub file_paths: HashMap<PathBuf, usize>,
    pub difficulty_histogram: Histogram<5>,
    pub retrievability_histogram: Histogram<5>,
}

#[derive(Debug, Clone)]
pub struct Histogram<const N: usize> {
    pub bins: [u32; N],
    count: u64,
    sum: f64,
}

impl<const N: usize> Default for Histogram<N> {
    #[inline]
    fn default() -> Self {
        Self {
            bins: [0; N],
            count: 0,
            sum: 0.0,
        }
    }
}
impl<const N: usize> Histogram<N> {
    pub fn update(&mut self, value: f64) {
        let v = value.clamp(0.0, 1.0);
        let mut idx = (v * N as f64) as usize;
        idx = idx.min(N - 1);
        self.bins[idx] += 1;
        self.count += 1;
        self.sum += value;
    }
    pub fn mean(&self) -> f64 {
        self.sum / self.count as f64
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum CardLifeCycle {
    New,
    Learning,
    Young,
    Mature,
}
const MATURE_INTERVAL: f64 = 21.0;

impl CardStats {
    // row is a Record
    pub fn update(&mut self, card: &Card, row: &CardStatsRow) {
        let review_stage = row.review_stage;
        let due_date = row.due_date;
        let interval = row.interval_raw.unwrap_or_default();
        let difficulty = row.difficulty.unwrap_or_default();
        let stability = row.stability.unwrap_or_default();
        let last_reviewed_at = row.last_reviewed_at;

        let now = chrono::Utc::now();
        let week_horizon = now + chrono::Duration::days(7);
        let month_horizon = now + chrono::Duration::days(30);
        *self.file_paths.entry(card.file_path.clone()).or_insert(0) += 1;

        let lifecycle = if review_stage == ReviewStage::New {
            CardLifeCycle::New
        } else if review_stage == ReviewStage::LearningA || review_stage == ReviewStage::LearningB {
            CardLifeCycle::Learning
        } else if interval > MATURE_INTERVAL {
            CardLifeCycle::Mature
        } else {
            CardLifeCycle::Young
        };

        *self.card_lifecycles.entry(lifecycle).or_insert(0) += 1;

        match due_date {
            None => {
                self.due_cards += 1;
                let day = now.format("%Y-%m-%d").to_string();
                *self.upcoming_week.entry(day).or_insert(0) += 1;
                self.upcoming_month += 1;
            }
            Some(due_date) => {
                if due_date <= now {
                    self.due_cards += 1;
                    let day = now.format("%Y-%m-%d").to_string();
                    *self.upcoming_week.entry(day).or_insert(0) += 1;
                    self.upcoming_month += 1;
                } else {
                    if due_date <= week_horizon {
                        let day = due_date.format("%Y-%m-%d").to_string();
                        *self.upcoming_week.entry(day).or_insert(0) += 1;
                    }

                    if due_date <= month_horizon {
                        self.upcoming_month += 1;
                    }
                }
            }
        }
        self.difficulty_histogram.update(difficulty / 10.0);
        let Some(last_reviewed_at) = last_reviewed_at else {
            return;
        };

        let elapsed_days =
            now.signed_duration_since(last_reviewed_at).num_seconds() as f64 / 86_400.0;
        let retrievabiliity = calculate_recall(elapsed_days.max(0.0), stability);
        self.retrievability_histogram.update(retrievabiliity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, CardContent};
    use chrono::{Duration, Utc};

    fn sample_card(path: &str) -> Card {
        Card {
            file_path: PathBuf::from(path),
            file_card_range: (0, 1),
            content: CardContent::Basic {
                question: "Q".into(),
                answer: "A".into(),
            },
            card_hash: "hash".into(),
        }
    }

    fn default_row() -> CardStatsRow {
        CardStatsRow {
            card_hash: "hash".into(),
            review_count: 0,
            due_date: None,
            interval_raw: None,
            difficulty: None,
            stability: None,
            last_reviewed_at: None,
        }
    }

    #[test]
    fn counts_new_card_as_due_and_new() {
        let mut stats = CardStats::default();
        let card = sample_card("deck/file.md");
        let mut row = default_row();
        row.difficulty = Some(5.0);

        stats.update(&card, &row);

        assert_eq!(*stats.card_lifecycles.get(&CardLifeCycle::New).unwrap(), 1);
        assert_eq!(stats.due_cards, 1);
        assert_eq!(stats.upcoming_month, 1);
        assert_eq!(stats.file_paths.get(&card.file_path), Some(&1));
        assert_eq!(stats.difficulty_histogram.bins[2], 1);
    }

    #[test]
    fn marks_mature_future_due_cards_correctly() {
        let mut stats = CardStats::default();
        let card = sample_card("deck/file.md");
        let mut row = default_row();
        row.review_count = 5;
        row.interval_raw = Some(30.0);
        row.due_date = Some(Utc::now() + Duration::days(3));

        stats.update(&card, &row);

        assert_eq!(
            *stats.card_lifecycles.get(&CardLifeCycle::Mature).unwrap(),
            1
        );
        assert_eq!(stats.due_cards, 0);
        assert_eq!(stats.upcoming_month, 1);
        assert_eq!(stats.upcoming_week.values().sum::<usize>(), 1);
    }

    #[test]
    fn updates_retrievability_histogram_when_reviewed() {
        let mut stats = CardStats::default();
        let card = sample_card("deck/file.md");
        let mut row = default_row();
        row.review_count = 2;
        row.interval_raw = Some(5.0);
        row.stability = Some(5.0);
        row.last_reviewed_at = Some(Utc::now() - Duration::days(4));

        stats.update(&card, &row);

        let recall = calculate_recall(4.0, 5.0);
        let idx = ((recall.clamp(0.0, 1.0) * 5.0) as usize).min(4);
        assert_eq!(stats.retrievability_histogram.bins[idx], 1);
    }
}
