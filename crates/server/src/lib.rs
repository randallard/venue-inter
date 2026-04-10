pub mod audit;
pub mod auth;
pub mod db;
pub mod api;
pub mod documents;
pub mod reviews;
pub mod tasks;
pub mod task_runner;
pub mod email;
pub mod tickets;

pub use shared_types;
pub use odbc_api;

use auth::types::{ConfiguredClient, OidcConfig};
use db::{AppConfig, EmailConfig};
use odbc_api::Environment;
use tokio::sync::RwLock;

/// Shared application state passed via Axum's `State` extractor.
pub struct AppState {
    pub env: Environment,
    pub config: AppConfig,
    /// None until OIDC discovery succeeds (may be deferred past startup).
    pub oidc_client: RwLock<Option<ConfiguredClient>>,
    pub oidc_config: OidcConfig,
    pub http_client: reqwest::Client,
    pub pg_pool: Option<sqlx::PgPool>,
    pub email_config: Option<EmailConfig>,
    pub webdav_config: Option<db::WebDavConfig>,
}
