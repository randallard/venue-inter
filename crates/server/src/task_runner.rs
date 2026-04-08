use std::sync::Arc;

use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

use crate::AppState;
use shared_types::ReplaceStaffParams;

/// Shared failure handler: marks the task failed, creates a support ticket,
/// and emails the sysadmin + user if SMTP is configured.
pub async fn handle_task_failure(
    state: &Arc<AppState>,
    pg_pool: &PgPool,
    task_id: Uuid,
    user_sub: &str,
    user_email: Option<&str>,
    task_description: &str,
    error_msg: &str,
) {
    if let Err(e) =
        crate::tasks::update_task_status(pg_pool, task_id, "failed", None, Some(error_msg)).await
    {
        error!(error = %e, "Failed to update task status to failed");
    }

    if let Err(e) = crate::tickets::create_ticket(
        pg_pool,
        task_id,
        user_sub,
        user_email,
        &format!("Task failed: {error_msg}"),
    )
    .await
    {
        error!(error = %e, "Failed to create failure ticket");
    }

    if let Some(ref email_config) = state.email_config {
        if let Err(e) = crate::email::send_failure_email(
            email_config,
            user_email,
            error_msg,
            task_description,
        )
        .await
        {
            error!(error = %e, "Failed to send failure email");
        }
    } else {
        tracing::warn!("SMTP not configured — skipping failure email");
    }
}

pub fn spawn_replace_staff_task(
    state: Arc<AppState>,
    pg_pool: PgPool,
    task_id: Uuid,
    params: ReplaceStaffParams,
    user_sub: String,
    user_email: Option<String>,
) {
    tokio::spawn(async move {
        info!(task_id = %task_id, "Starting replace staff task");

        if let Err(e) = crate::tasks::update_task_status(&pg_pool, task_id, "running", None, None).await {
            error!(error = %e, "Failed to update task status to running");
        }

        let result = tokio::task::spawn_blocking({
            let state = state.clone();
            let params = params.clone();
            move || -> Result<usize, String> {
                let conn = state.env
                    .connect(
                        &state.config.dsn,
                        &state.config.user,
                        &state.config.password,
                        ConnectionOptions::default(),
                    )
                    .map_err(|e| format!("DB connection failed: {e}"))?;

                // Count matching rows first
                let count_sql = format!(
                    "SELECT COUNT(*) FROM session_resources WHERE sr_name = '{}' AND sr_type = '{}'",
                    params.old_name.replace('\'', "''"),
                    params.ct_type.replace('\'', "''"),
                );

                let count = {
                    let mut stmt = conn.execute(&count_sql, ())
                        .map_err(|e| format!("Count query failed: {e}"))?
                        .ok_or_else(|| "No result from count query".to_string())?;
                    let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(256))
                        .map_err(|e| format!("Buffer error: {e}"))?;
                    let mut cursor = stmt.bind_buffer(&mut buf)
                        .map_err(|e| format!("Bind error: {e}"))?;
                    match cursor.fetch().map_err(|e| format!("Fetch error: {e}"))? {
                        Some(batch) => batch.at(0, 0)
                            .and_then(|b| String::from_utf8_lossy(b).trim().parse::<usize>().ok())
                            .unwrap_or(0),
                        None => 0,
                    }
                };

                if count == 0 {
                    return Ok(0);
                }

                let update_sql = format!(
                    "UPDATE session_resources SET sr_name = '{}' WHERE sr_name = '{}' AND sr_type = '{}'",
                    params.new_name.replace('\'', "''"),
                    params.old_name.replace('\'', "''"),
                    params.ct_type.replace('\'', "''"),
                );

                conn.execute(&update_sql, ())
                    .map_err(|e| format!("UPDATE failed: {e}"))?;

                Ok(count)
            }
        })
        .await;

        let error_msg = match result {
            Ok(Ok(count)) => {
                let summary = format!("Updated {} record(s): '{}' -> '{}' (type: {})",
                    count, params.old_name, params.new_name, params.ct_type);
                info!(task_id = %task_id, count, "Replace staff task completed");

                if let Err(e) = crate::tasks::update_task_status(
                    &pg_pool, task_id, "completed", Some(&summary), None
                ).await {
                    error!(error = %e, "Failed to update task status to completed");
                }
                None
            }
            Ok(Err(e)) => Some(e),
            Err(e) => Some(format!("Task panicked: {e}")),
        };

        if let Some(error_msg) = error_msg {
            error!(task_id = %task_id, error = %error_msg, "Replace staff task failed");

            handle_task_failure(
                &state,
                &pg_pool,
                task_id,
                &user_sub,
                user_email.as_deref(),
                &format!("Replace {} '{}' with '{}'", params.ct_type, params.old_name, params.new_name),
                &error_msg,
            )
            .await;
        }
    });
}
