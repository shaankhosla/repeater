use serde::{Deserialize, Serialize};

/// Last-write-wins: returns true if `remote` should replace `local`.
pub fn remote_wins(remote: &Option<String>, local: &Option<String>) -> bool {
    match (remote, local) {
        (Some(r), Some(l)) => r > l,
        (Some(_), None) => true,
        _ => false,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCard {
    pub card_hash: String,
    pub last_reviewed_at: Option<String>,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub interval_raw: Option<f64>,
    pub interval_days: Option<i64>,
    pub due_date: Option<String>,
    pub review_count: i64,
    pub added_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushRequest {
    pub cards: Vec<SyncCard>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub updated: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub cards: Vec<SyncCard>,
    pub latest_version: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStatus {
    pub card_count: i64,
    pub latest_version: i64,
}

#[cfg(test)]
mod tests {
    use super::remote_wins;

    #[test]
    fn remote_newer_wins() {
        assert!(remote_wins(
            &Some("2026-03-29T12:00:00+00:00".into()),
            &Some("2026-03-29T06:00:00+00:00".into()),
        ));
    }

    #[test]
    fn local_newer_wins() {
        assert!(!remote_wins(
            &Some("2026-03-29T06:00:00+00:00".into()),
            &Some("2026-03-29T12:00:00+00:00".into()),
        ));
    }

    #[test]
    fn remote_some_local_none() {
        assert!(remote_wins(&Some("2026-03-29T12:00:00+00:00".into()), &None));
    }

    #[test]
    fn remote_none_local_some() {
        assert!(!remote_wins(&None, &Some("2026-03-29T12:00:00+00:00".into())));
    }

    #[test]
    fn both_none() {
        assert!(!remote_wins(&None, &None));
    }
}
