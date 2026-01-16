use crate::{crud::DB, palette::Palette};

use std::{
    io::{self, Write},
    time::Duration,
};

use anyhow::Result;
use serde::Deserialize;

const TIMEOUT: u64 = 900;
pub const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);
pub const ONE_WEEK: Duration = Duration::from_secs(60 * 60 * 24 * 7);

#[derive(Deserialize, Debug)]
struct Release {
    tag_name: String,
}

#[derive(Debug, Clone)]
pub struct VersionNotification {
    pub current_version: String,
    pub latest_version: String,
}

#[derive(Debug, Clone, Default)]
pub struct VersionUpdateStats {
    pub last_prompted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_version_check_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn should_notify(now: chrono::DateTime<chrono::Utc>, stats: &VersionUpdateStats) -> bool {
    if let Some(last_check) = stats.last_version_check_at
        && now.signed_duration_since(last_check) < chrono::Duration::from_std(ONE_DAY).unwrap()
    {
        return false;
    }

    if let Some(last_prompted) = stats.last_prompted_at
        && now.signed_duration_since(last_prompted) < chrono::Duration::from_std(ONE_WEEK).unwrap()
    {
        return false;
    }

    true
}

pub async fn check_version(db: DB) -> Option<VersionNotification> {
    let now = chrono::Utc::now();
    let version_update_stats = db.get_version_update_information().await.ok()?;

    if !should_notify(now, &version_update_stats) {
        return None;
    }

    let current_version = env!("CARGO_PKG_VERSION");
    let latest_release = get_latest().await.ok()?;

    #[cfg(debug_assertions)]
    {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(now)
            .num_milliseconds();

        dbg!(elapsed, "ms");
    }

    db.update_last_version_check_at().await.ok();

    if normalize_version(&latest_release.tag_name) == normalize_version(current_version) {
        return None;
    }

    Some(VersionNotification {
        current_version: current_version.to_string(),
        latest_version: normalize_version(latest_release.tag_name.as_str()),
    })
}

async fn get_latest() -> Result<Release> {
    let client = reqwest::Client::new();

    const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

    let release: Release = client
        .get("https://api.github.com/repos/shaankhosla/repeater/releases/latest")
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_millis(TIMEOUT))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(release)
}

pub async fn prompt_for_new_version(db: &DB, notification: &VersionNotification) {
    db.update_last_prompted_at().await.ok();

    println!(
        "\nA new version of {} is available! {} -> {}",
        Palette::paint(Palette::INFO, "repeater"),
        Palette::paint(Palette::DANGER, &notification.current_version),
        Palette::paint(Palette::SUCCESS, &notification.latest_version)
    );

    println!(
        "Check {} for more details",
        Palette::paint(
            Palette::ACCENT,
            "https://github.com/shaankhosla/repeater/releases"
        )
    );

    println!(
        "{}",
        Palette::dim("Press any key to dismiss (I'll remind you again in a few days)")
    );
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches(['v', 'V']).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("v1.0.0"), "1.0.0");
        assert_eq!(normalize_version("1.0.0"), "1.0.0");
    }
    #[test]
    fn should_notify_when_never_checked_or_prompted() {
        let now = chrono::Utc::now();
        let stats = VersionUpdateStats::default();

        assert!(should_notify(now, &stats));
    }

    #[test]
    fn should_not_notify_if_checked_within_one_day() {
        let now = chrono::Utc::now();
        let stats = VersionUpdateStats {
            last_version_check_at: Some(now - chrono::Duration::hours(12)),
            last_prompted_at: None,
        };

        assert!(!should_notify(now, &stats));
    }

    #[test]
    fn should_not_notify_if_prompted_within_one_week() {
        let now = chrono::Utc::now();
        let stats = VersionUpdateStats {
            last_version_check_at: Some(now - chrono::Duration::days(2)),
            last_prompted_at: Some(now - chrono::Duration::days(3)),
        };

        assert!(!should_notify(now, &stats));
    }

    #[test]
    fn should_notify_if_both_are_old() {
        let now = chrono::Utc::now();
        let stats = VersionUpdateStats {
            last_version_check_at: Some(now - chrono::Duration::days(2)),
            last_prompted_at: Some(now - chrono::Duration::days(10)),
        };

        assert!(should_notify(now, &stats));
    }
}
