use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use tracing::debug;

use shared_types::UserSession;

use super::routes::USER_SESSION_KEY;

/// Sessions are valid for at most 12 hours regardless of activity (FISMA).
const MAX_SESSION_SECS: i64 = 12 * 3600;

/// Extract the client IP from X-Forwarded-For or X-Real-IP headers.
/// Returns the first (leftmost) address in X-Forwarded-For, which is the
/// original client when behind a trusted reverse proxy.
pub fn extract_ip(request: &Request) -> Option<String> {
    request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| {
            request
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
}

/// Extract the x-request-id header value (set by SetRequestIdLayer).
pub fn extract_request_id(request: &Request) -> Option<String> {
    request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Middleware that requires an authenticated, non-expired session.
///
/// - Browsers are redirected to /auth/login?return_to=<path>
/// - API requests receive 401 Unauthorized
/// - Sessions older than MAX_SESSION_SECS are flushed and rejected
pub async fn require_auth(session: Session, mut request: Request, next: Next) -> Response {
    let ip = extract_ip(&request);
    let request_id = extract_request_id(&request);
    let path = request.uri().path().to_string();
    let method = request.method().to_string();

    match session.get::<UserSession>(USER_SESSION_KEY).await {
        Ok(Some(user)) => {
            // Enforce absolute session lifetime (FISMA AC-12)
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if now - user.authenticated_at > MAX_SESSION_SECS {
                crate::audit::session_expired(
                    &user.sub,
                    &path,
                    ip.as_deref(),
                    request_id.as_deref(),
                );
                let _ = session.flush().await;
                return if path.starts_with("/api/") {
                    StatusCode::UNAUTHORIZED.into_response()
                } else {
                    Redirect::to(&format!("/auth/login?return_to={path}")).into_response()
                };
            }

            debug!(sub = %user.sub, path, "Authenticated request");
            crate::audit::access_granted(
                &user.sub,
                &method,
                &path,
                request_id.as_deref(),
            );
            request.extensions_mut().insert(user);
            next.run(request).await
        }
        Ok(None) => {
            crate::audit::session_missing(&path, ip.as_deref(), request_id.as_deref());
            if path.starts_with("/api/") {
                StatusCode::UNAUTHORIZED.into_response()
            } else {
                Redirect::to(&format!("/auth/login?return_to={path}")).into_response()
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to read session");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
