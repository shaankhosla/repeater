use std::env;

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub db_uri: String,
    pub open_registration: bool,
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            host: env::var("REPEATER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("REPEATER_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            db_uri: env::var("REPEATER_DB_URI")
                .unwrap_or_else(|_| "postgres://localhost/repeater".to_string()),
            open_registration: env::var("REPEATER_OPEN_REGISTRATION")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
