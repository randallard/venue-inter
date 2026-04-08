use anyhow::{Context, Result};
use openidconnect::{
    core::CoreClient, EndpointMaybeSet, EndpointNotSet, EndpointSet,
};
use std::env;

// Re-export UserSession from shared-types so downstream code can use server::auth::types::UserSession
pub use shared_types::UserSession;

/// CoreClient with endpoint types matching what `from_provider_metadata` returns.
pub type ConfiguredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub struct OidcConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub ca_cert_path: Option<String>,
}

impl OidcConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            issuer_url: env::var("OIDC_ISSUER_URL").context("OIDC_ISSUER_URL must be set")?,
            client_id: env::var("OIDC_CLIENT_ID").context("OIDC_CLIENT_ID must be set")?,
            client_secret: env::var("OIDC_CLIENT_SECRET")
                .context("OIDC_CLIENT_SECRET must be set")?,
            redirect_uri: env::var("OIDC_REDIRECT_URI")
                .context("OIDC_REDIRECT_URI must be set")?,
            ca_cert_path: env::var("OIDC_CA_CERT_PATH").ok(),
        })
    }
}
