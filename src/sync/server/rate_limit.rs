use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use tokio::sync::Mutex;

use std::net::SocketAddr;

#[derive(Clone)]
pub struct RateLimiter {
    max_requests: u32,
    window_secs: u64,
    state: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            state: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub async fn rate_limit_middleware(
    State(limiter): State<RateLimiter>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip();
    let now = Instant::now();

    let mut state = limiter.state.lock().await;
    let entries = state.entry(ip).or_default();

    // Remove entries outside the window
    let cutoff = now - std::time::Duration::from_secs(limiter.window_secs);
    entries.retain(|&t| t > cutoff);

    if entries.len() >= limiter.max_requests as usize {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    entries.push(now);
    drop(state);

    Ok(next.run(request).await)
}
