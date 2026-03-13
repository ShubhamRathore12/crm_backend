//! SMC CRM Backend — Axum API server

use axum::{Router, routing::get};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db_manager;
mod config;
mod error;
mod api;
mod workflow_engine;
mod assignment_engine;
mod websocket;
mod ai_scoring;

use config::Config;
use error::AppError;
use websocket::WebSocketManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    let secondary_pool = if let Some(url) = &config.secondary_database_url {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await 
        {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::error!("Failed to connect to secondary database: {}", e);
                None
            }
        }
    } else {
        None
    };

    let db = db_manager::DbManager::new(pool.clone(), secondary_pool);
    let websocket_manager = WebSocketManager::new();

    let app = Router::new()
        .route("/health", get(health))
        .nest("/auth", api::auth::routes())
        .nest("/leads", api::leads::routes())
        .nest("/contacts", api::contacts::routes())
        .nest("/interactions", api::interactions::routes())
        .nest("/sms", api::messaging::sms_routes())
        .nest("/email", api::messaging::email_routes())
        .nest("/email-inbound", api::email_inbound::routes())
        .nest("/whatsapp", api::messaging::whatsapp_routes())
        .nest("/cti", api::cti::routes())
        .nest("/workflow", api::workflow::routes())
        .nest("/integrations", api::integrations::routes())
        .nest("/ai", api::ai_scoring::routes())
        .nest("/sales-marketing", api::sales_marketing::routes())
        .nest("/sales-marketing/forms", api::sales_forms::routes())
        .nest("/opportunities", api::opportunities::routes())
        .nest("/attachments", api::attachments::routes())
        .nest("/bulk-uploads", api::bulk_uploads::routes())
        .nest("/fields", api::field_definitions::routes())
        .nest("/maintenance", api::maintenance::routes())
        .nest("/teams", api::teams::routes())
        .nest("/analytics", api::analytics::routes())
        .nest("/users", api::users::routes())
        .nest("/ws", websocket::routes())
        .with_state(AppState { 
            db,
            pool, 
            config: config.clone(),
            websocket_manager,
        })
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

#[derive(Clone)]
pub struct AppState {
    pub db: db_manager::DbManager,
    pub pool: sqlx::PgPool, // Keeping for compatibility
    pub config: Config,
    pub websocket_manager: WebSocketManager,
}
