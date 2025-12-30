use crate::crud::DB;

use std::{
    io::{self, Write},
    time::Duration,
};

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Release {
    tag_name: String,
}

#[derive(Debug, Clone)]
pub struct VersionNotification {
    pub current_version: String,
    pub latest_version: String,
}

pub async fn check_version(_db: &DB) -> Option<VersionNotification> {
    let current_version = env!("CARGO_PKG_VERSION");
    let latest_release = get_latest().await.ok()?;

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
        .get("https://api.github.com/repos/shaankhosla/repeat/releases/latest")
        .header("User-Agent", USER_AGENT)
        .timeout(Duration::from_millis(600))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(release)
}

pub fn prompt_for_new_version(notification: &VersionNotification) {
    let dim = "\x1b[2m";
    let reset = "\x1b[0m";
    let cyan = "\x1b[36m";
    let red = "\x1b[31m";
    let green = "\x1b[32m";
    let blue = "\x1b[34m";

    println!(
        "\nA new version of {cyan}repeat{reset} is available! \
         {red}{}{reset} -> {green}{}{reset}",
        notification.current_version, notification.latest_version
    );

    println!("Check {blue}https://github.com/shaankhosla/repeat/releases{reset} for more details");

    println!("{dim}Press any key to continue (I'll remind you again in a few days){reset}");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

fn normalize_version(version: &str) -> String {
    version.trim().trim_start_matches('v').to_string()
}
