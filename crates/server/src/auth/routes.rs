use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreProviderMetadata},
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::Deserialize;
use tower_sessions::Session;
use base64::Engine;
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::types::{ConfiguredClient, OidcConfig};
use shared_types::UserSession;

/// Perform OIDC provider discovery and build a configured client.
/// Extracted so it can be called both at startup and lazily on first login.
pub async fn discover_oidc_client(
    config: &OidcConfig,
    http_client: &reqwest::Client,
) -> anyhow::Result<ConfiguredClient> {
    let issuer_url = IssuerUrl::new(config.issuer_url.clone())
        .context("Invalid OIDC issuer URL")?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, http_client)
        .await
        .context("OIDC provider discovery failed")?;
    let client = openidconnect::core::CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(config.client_id.clone()),
        Some(ClientSecret::new(config.client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.redirect_uri.clone()).context("Invalid OIDC redirect URI")?,
    );
    Ok(client)
}

const PKCE_VERIFIER_KEY: &str = "pkce_verifier";
const CSRF_STATE_KEY: &str = "csrf_state";
const NONCE_KEY: &str = "nonce";
pub const USER_SESSION_KEY: &str = "user";
const RETURN_TO_KEY: &str = "return_to";

#[derive(Deserialize)]
pub struct LoginParams {
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
}

fn extract_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

fn extract_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// GET /auth/login — starts the OIDC authorization code flow
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<LoginParams>,
) -> Result<Response, StatusCode> {
    let ip = extract_ip(&headers);
    let request_id = extract_request_id(&headers);

    // Lazy OIDC discovery: if startup discovery failed, try again on first login.
    {
        let guard = state.oidc_client.read().await;
        if guard.is_none() {
            drop(guard);
            info!("OIDC client not available — attempting lazy discovery");
            match discover_oidc_client(&state.oidc_config, &state.http_client).await {
                Ok(client) => {
                    *state.oidc_client.write().await = Some(client);
                    info!("OIDC client configured via lazy discovery");
                }
                Err(e) => {
                    error!(error = %e, "Lazy OIDC discovery failed — login unavailable");
                    return Err(StatusCode::SERVICE_UNAVAILABLE);
                }
            }
        }
    }

    let client_guard = state.oidc_client.read().await;
    let oidc_client = client_guard.as_ref().unwrap();

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = oidc_client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .add_scope(Scope::new("groups".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    session
        .insert(PKCE_VERIFIER_KEY, pkce_verifier.secret().to_string())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store PKCE verifier in session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    session
        .insert(CSRF_STATE_KEY, csrf_token.secret().to_string())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store CSRF state in session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    session
        .insert(NONCE_KEY, nonce.secret().to_string())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store nonce in session");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store return_to URL — only allow relative paths to prevent open redirect
    if let Some(ref path) = params.return_to {
        if path.starts_with('/') && !path.starts_with("//") {
            session.insert(RETURN_TO_KEY, path.clone()).await.ok();
        }
    }

    crate::audit::login_initiated(ip.as_deref(), request_id.as_deref());
    info!("Redirecting to OIDC provider for authentication");
    Ok(Redirect::to(auth_url.as_str()).into_response())
}

/// GET /auth/callback — handles the OIDC provider redirect
pub async fn callback_handler(
    State(state): State<Arc<AppState>>,
    session: Session,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Result<Response, (StatusCode, String)> {
    let ip = extract_ip(&headers);
    let request_id = extract_request_id(&headers);

    // Validate CSRF state
    let stored_state: String = session
        .get(CSRF_STATE_KEY)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to read CSRF state from session");
            (StatusCode::INTERNAL_SERVER_ERROR, "Session read error".to_string())
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Missing CSRF state in session — start login again".to_string())
        })?;

    if params.state != stored_state {
        crate::audit::login_failure("csrf_mismatch", ip.as_deref(), request_id.as_deref());
        return Err((StatusCode::BAD_REQUEST, "CSRF state mismatch".to_string()));
    }

    let pkce_secret: String = session
        .get(PKCE_VERIFIER_KEY)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to read PKCE verifier from session");
            (StatusCode::INTERNAL_SERVER_ERROR, "Session read error".to_string())
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Missing PKCE verifier in session".to_string())
        })?;

    let nonce_secret: String = session
        .get(NONCE_KEY)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to read nonce from session");
            (StatusCode::INTERNAL_SERVER_ERROR, "Session read error".to_string())
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, "Missing nonce in session".to_string())
        })?;

    let pkce_verifier = PkceCodeVerifier::new(pkce_secret);
    let nonce = Nonce::new(nonce_secret);

    let client_guard = state.oidc_client.read().await;
    let oidc_client = client_guard.as_ref().ok_or_else(|| {
        error!("OIDC callback received but client is not configured");
        (StatusCode::SERVICE_UNAVAILABLE, "Authentication service unavailable".to_string())
    })?;

    let token_response = oidc_client
        .exchange_code(AuthorizationCode::new(params.code))
        .map_err(|e| {
            error!(error = %e, "Token endpoint not configured");
            (StatusCode::INTERNAL_SERVER_ERROR, "Token endpoint not configured".to_string())
        })?
        .set_pkce_verifier(pkce_verifier)
        .request_async(&state.http_client)
        .await
        .map_err(|e| {
            crate::audit::login_failure("token_exchange_failed", ip.as_deref(), request_id.as_deref());
            error!(error = %e, "Token exchange failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "Token exchange failed".to_string())
        })?;

    let id_token = token_response.id_token().ok_or_else(|| {
        crate::audit::login_failure("no_id_token", ip.as_deref(), request_id.as_deref());
        error!("No ID token in response");
        (StatusCode::INTERNAL_SERVER_ERROR, "No ID token in response".to_string())
    })?;

    let claims = id_token
        .claims(&oidc_client.id_token_verifier(), &nonce)
        .map_err(|e| {
            crate::audit::login_failure("id_token_validation_failed", ip.as_deref(), request_id.as_deref());
            error!(error = %e, "ID token validation failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "ID token validation failed".to_string())
        })?;

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let user_session = UserSession {
        sub: claims.subject().to_string(),
        email: claims.email().map(|e| e.to_string()),
        name: claims
            .name()
            .and_then(|n| n.get(None))
            .map(|n| n.to_string()),
        groups: extract_groups_from_id_token(id_token),
        authenticated_at: now,
    };

    crate::audit::login_success(
        &user_session.sub,
        user_session.email.as_deref(),
        ip.as_deref(),
        request_id.as_deref(),
    );
    info!(sub = %user_session.sub, email = ?user_session.email, "User authenticated");

    // Clean up auth state
    let _ = session.remove::<String>(PKCE_VERIFIER_KEY).await;
    let _ = session.remove::<String>(CSRF_STATE_KEY).await;
    let _ = session.remove::<String>(NONCE_KEY).await;

    // Retrieve and validate return_to before inserting user session
    let return_to = session
        .remove::<String>(RETURN_TO_KEY)
        .await
        .ok()
        .flatten()
        .filter(|p| p.starts_with('/') && !p.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());

    session.insert(USER_SESSION_KEY, user_session).await.map_err(|e| {
        error!(error = %e, "Failed to store user session");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to store session".to_string())
    })?;

    Ok(Redirect::to(&return_to).into_response())
}

/// POST /auth/logout — clears the local session and redirects to Authentik's
/// end-session endpoint so the IdP session is also invalidated.
pub async fn logout_handler(session: Session, headers: HeaderMap) -> Response {
    let ip = extract_ip(&headers);
    let request_id = extract_request_id(&headers);

    // Capture sub before flushing for audit log
    let user_sub = session
        .get::<UserSession>(USER_SESSION_KEY)
        .await
        .ok()
        .flatten()
        .map(|u| u.sub);

    if let Err(e) = session.flush().await {
        error!(error = %e, "Failed to flush session on logout");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    if let Some(ref sub) = user_sub {
        crate::audit::logout(sub, ip.as_deref(), request_id.as_deref());
        info!(sub, "User logged out");
    }

    let redirect_url = match std::env::var("OIDC_ISSUER_URL") {
        Ok(issuer) => {
            let base = issuer.trim_end_matches('/');
            format!("{base}/end-session/?redirect_uri=http%3A%2F%2Flocalhost%3A8080%2F")
        }
        Err(_) => "/".to_string(),
    };

    Redirect::to(&redirect_url).into_response()
}

/// Extract group names from the ID token's "groups" claim.
/// Authentik embeds groups as a JSON array in the token payload.
fn extract_groups_from_id_token(
    id_token: &openidconnect::IdToken<
        openidconnect::EmptyAdditionalClaims,
        openidconnect::core::CoreGenderClaim,
        openidconnect::core::CoreJweContentEncryptionAlgorithm,
        openidconnect::core::CoreJwsSigningAlgorithm,
    >,
) -> Vec<String> {
    let token_str = id_token.to_string();
    let parts: Vec<&str> = token_str.split('.').collect();
    if parts.len() < 2 {
        warn!("ID token has unexpected format — cannot extract groups");
        return Vec::new();
    }

    let payload_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(b) => b,
        Err(e) => {
            warn!(error = %e, "Failed to base64-decode ID token payload");
            return Vec::new();
        }
    };

    let payload: serde_json::Value = match serde_json::from_slice(&payload_bytes) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Failed to parse ID token payload as JSON");
            return Vec::new();
        }
    };

    match payload.get("groups") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => {
            info!("No 'groups' claim found in ID token");
            Vec::new()
        }
    }
}

use std::time::SystemTime;
