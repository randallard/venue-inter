//! Structured audit logging for FISMA High compliance.
//!
//! All events emit under the `audit` tracing target so they can be routed
//! independently from operational logs:
//!
//!   RUST_LOG=info,audit=info
//!
//! In production, pipe the `audit` target into a tamper-evident, append-only
//! store (e.g. a WORM log aggregator or a write-once S3 bucket).
//!
//! Every event includes: UTC timestamp (from tracing), event type, and
//! context-specific fields. The `request_id` field ties audit events to
//! the x-request-id header visible in application logs and HTTP responses.

use tracing::info;

/// Authentication flow started — login redirect issued to OIDC provider.
pub fn login_initiated(ip: Option<&str>, request_id: Option<&str>) {
    info!(
        target: "audit",
        event = "auth.login_initiated",
        ip,
        request_id,
    );
}

/// Authentication succeeded — OIDC callback validated, user session created.
pub fn login_success(
    user_sub: &str,
    email: Option<&str>,
    ip: Option<&str>,
    request_id: Option<&str>,
) {
    info!(
        target: "audit",
        event = "auth.login_success",
        user_sub,
        email,
        ip,
        request_id,
    );
}

/// Authentication failed during OIDC callback (CSRF mismatch, token error, etc).
pub fn login_failure(reason: &str, ip: Option<&str>, request_id: Option<&str>) {
    info!(
        target: "audit",
        event = "auth.login_failure",
        reason,
        ip,
        request_id,
    );
}

/// User logged out — session flushed.
pub fn logout(user_sub: &str, ip: Option<&str>, request_id: Option<&str>) {
    info!(
        target: "audit",
        event = "auth.logout",
        user_sub,
        ip,
        request_id,
    );
}

/// Request rejected: no session found for the path.
pub fn session_missing(path: &str, ip: Option<&str>, request_id: Option<&str>) {
    info!(
        target: "audit",
        event = "auth.session_missing",
        path,
        ip,
        request_id,
    );
}

/// Request rejected: session exceeded the 12-hour absolute lifetime.
pub fn session_expired(
    user_sub: &str,
    path: &str,
    ip: Option<&str>,
    request_id: Option<&str>,
) {
    info!(
        target: "audit",
        event = "auth.session_expired",
        user_sub,
        path,
        ip,
        request_id,
    );
}

/// Access granted to a protected resource.
pub fn access_granted(
    user_sub: &str,
    method: &str,
    path: &str,
    request_id: Option<&str>,
) {
    info!(
        target: "audit",
        event = "access.granted",
        user_sub,
        method,
        path,
        request_id,
    );
}

/// Background task created by an authenticated user.
pub fn task_created(
    user_sub: &str,
    task_id: &str,
    task_type: &str,
    description: &str,
    request_id: Option<&str>,
) {
    info!(
        target: "audit",
        event = "task.created",
        user_sub,
        task_id,
        task_type,
        description,
        request_id,
    );
}

/// Administrative endpoint accessed (helpdesk/all-tickets).
pub fn admin_access(user_sub: &str, endpoint: &str, request_id: Option<&str>) {
    info!(
        target: "audit",
        event = "admin.access",
        user_sub,
        endpoint,
        request_id,
    );
}
