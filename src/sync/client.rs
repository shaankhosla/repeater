use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::crud::DB;
use crate::palette::Palette;

use super::config::SyncConfig;
use super::types::{
    AuthRequest, AuthResponse, PullResponse, PushRequest, PushResponse, SyncStatus,
};

const SYNC_TIMEOUT: Duration = Duration::from_secs(5);

pub struct SyncClient {
    http: reqwest::Client,
    config: SyncConfig,
}

impl SyncClient {
    pub fn new(config: SyncConfig) -> Result<Self> {
        let http = reqwest::Client::builder().timeout(SYNC_TIMEOUT).build()?;
        Ok(Self { http, config })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.address.trim_end_matches('/'), path)
    }

    fn auth_header(&self) -> Result<String> {
        match &self.config.session_token {
            Some(token) => Ok(format!("Token {}", token)),
            None => bail!("Not logged in. Run `repeater sync login` first."),
        }
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<AuthResponse> {
        let resp = self
            .http
            .post(self.url("/register"))
            .json(&AuthRequest {
                username: username.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .context("Failed to connect to sync server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Registration failed ({}): {}", status, body);
        }

        resp.json()
            .await
            .context("Failed to parse register response")
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<AuthResponse> {
        let resp = self
            .http
            .post(self.url("/login"))
            .json(&AuthRequest {
                username: username.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .context("Failed to connect to sync server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Login failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse login response")
    }

    pub async fn push(&self, request: PushRequest) -> Result<PushResponse> {
        let resp = self
            .http
            .post(self.url("/sync/push"))
            .header("Authorization", self.auth_header()?)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to sync server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Push failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse push response")
    }

    pub async fn pull(&self, since_version: i64) -> Result<PullResponse> {
        let url = format!(
            "{}/sync/pull?since_version={}",
            self.config.address.trim_end_matches('/'),
            since_version
        );
        let resp = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header()?)
            .send()
            .await
            .context("Failed to connect to sync server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Pull failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse pull response")
    }

    pub async fn status(&self) -> Result<SyncStatus> {
        let resp = self
            .http
            .get(self.url("/sync/status"))
            .header("Authorization", self.auth_header()?)
            .send()
            .await
            .context("Failed to connect to sync server")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Status check failed ({}): {}", status, body);
        }

        resp.json().await.context("Failed to parse status response")
    }
}

/// Run a full sync cycle: push local changes, then pull remote changes.
/// Returns false if sync was skipped (not configured/logged in).
/// Fails silently with a warning if the server is unreachable.
pub async fn sync(db: &DB, quiet: bool) -> Result<bool> {
    let mut config = match SyncConfig::load() {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };

    if !config.is_logged_in() {
        return Ok(false);
    }

    let client = SyncClient::new(config.clone())?;

    // Push locally modified cards
    let modified_cards = db.get_locally_modified_cards().await?;
    if !modified_cards.is_empty() {
        let count = modified_cards.len();
        match client
            .push(PushRequest {
                cards: modified_cards,
            })
            .await
        {
            Ok(resp) => {
                db.clear_locally_modified().await?;
                if !quiet {
                    eprintln!(
                        "{}",
                        Palette::dim(format!(
                            "Sync: pushed {} cards ({} updated on server)",
                            count, resp.updated
                        ))
                    );
                }
            }
            Err(e) => {
                if !quiet {
                    eprintln!("{}", Palette::dim(format!("Sync push warning: {}", e)));
                }
                return Ok(true);
            }
        }
    }

    // Pull remote changes
    match client.pull(config.last_server_version).await {
        Ok(resp) => {
            let pulled_count = resp.cards.len();
            if !resp.cards.is_empty() {
                db.merge_pulled_cards(&resp.cards).await?;
            }
            config.last_server_version = resp.latest_version;
            config.save()?;
            if !quiet && pulled_count > 0 {
                eprintln!(
                    "{}",
                    Palette::dim(format!("Sync: pulled {} cards", pulled_count))
                );
            }
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}", Palette::dim(format!("Sync pull warning: {}", e)));
            }
        }
    }

    Ok(true)
}
