mod auth;
mod config;
mod db;
mod handlers;
mod rate_limit;

pub use config::ServerConfig;

use std::net::SocketAddr;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};

use self::db::ServerDB;

const BODY_LIMIT: usize = 5 * 1024 * 1024; // 5 MB

#[derive(Clone)]
pub struct AppState {
    db: ServerDB,
    config: ServerConfig,
}

// ServerConfig needs Clone for AppState
impl Clone for ServerConfig {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            port: self.port,
            db_uri: self.db_uri.clone(),
            open_registration: self.open_registration,
        }
    }
}

pub async fn start_server() -> anyhow::Result<()> {
    let config = ServerConfig::from_env();
    let bind_addr = config.bind_addr();

    eprintln!("Connecting to database: {}", config.db_uri);
    let db = ServerDB::new(&config.db_uri).await?;

    let state = AppState { db, config };

    let rate_limiter = rate_limit::RateLimiter::new(5, 60); // 5 requests per 60 seconds per IP

    let auth_routes = Router::new()
        .route("/register", post(auth::register))
        .route("/login", post(auth::login))
        .layer(middleware::from_fn_with_state(
            rate_limiter,
            rate_limit::rate_limit_middleware,
        ));

    let sync_routes = Router::new()
        .route("/sync/push", post(handlers::push))
        .route("/sync/pull", get(handlers::pull))
        .route("/sync/status", get(handlers::status));

    let app = Router::new()
        .merge(auth_routes)
        .merge(sync_routes)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .with_state(state);

    eprintln!("Repeater sync server listening on {}", bind_addr);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
