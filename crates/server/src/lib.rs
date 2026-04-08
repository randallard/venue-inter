pub mod audit;
pub mod auth;
pub mod db;
pub mod api;
pub mod reviews;
pub mod tasks;
pub mod task_runner;
pub mod email;
pub mod tickets;

pub use shared_types;
pub use odbc_api;

use auth::types::ConfiguredClient;
use db::{AppConfig, EmailConfig};
use odbc_api::Environment;

/// Shared application state passed via Axum's `State` extractor.
pub struct AppState {
    pub env: Environment,
    pub config: AppConfig,
    pub oidc_client: ConfiguredClient,
    pub http_client: reqwest::Client,
    pub pg_pool: Option<sqlx::PgPool>,
    pub email_config: Option<EmailConfig>,
}
