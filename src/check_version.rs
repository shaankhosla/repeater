use crate::crud::DB;

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

pub async fn check_version(db: DB) -> Option<VersionNotification> {
    let now = chrono::Utc::now();
    let version_update_stats = db.get_version_update_information().await.ok()?;

    if let Some(last_check) = version_update_stats.last_version_check_at
        && now.signed_duration_since(last_check) < chrono::Duration::from_std(ONE_DAY).ok()?
    {
        return None;
    }
    if let Some(last_prompted) = version_update_stats.last_prompted_at
        && now.signed_duration_since(last_prompted) < chrono::Duration::from_std(ONE_WEEK).ok()?
    {
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
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let cyan = "\x1b[36m";
    let red = "\x1b[31m";
    let green = "\x1b[32m";
    let blue = "\x1b[34m";

    println!(
        "\nA new version of {cyan}repeater{reset} is available! \
         {red}{}{reset} -> {green}{}{reset}",
        notification.current_version, notification.latest_version
    );

    println!(
        "Check {blue}https://github.com/shaankhosla/repeater/releases{reset} for more details"
    );

    println!("{dim}Press any key to dismiss (I'll remind you again in a few days){reset}");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}
