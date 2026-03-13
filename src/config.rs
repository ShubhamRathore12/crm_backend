//! Configuration from environment

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub secondary_database_url: Option<String>,
    pub redis_url: Option<String>,
    pub port: u16,
    pub jwt_secret: String,
    pub jwt_expiry_secs: i64,
    /// Base URL of the API (e.g. https://api.crm.example.com) for email tracking pixel and webhooks
    pub api_base_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/crm".into()),
            secondary_database_url: std::env::var("SECONDARY_DATABASE_URL").ok(),
            redis_url: std::env::var("REDIS_URL").ok(),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "change-me-in-production".into()),
            jwt_expiry_secs: std::env::var("JWT_EXPIRY_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(86400),
            api_base_url: std::env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".into()),
        })
    }
}
