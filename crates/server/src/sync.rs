//! Background synchronisation between PostgreSQL (app state) and Informix
//! (national system).
//!
//! Two processes:
//! 1. `process_sync_queue` — drains `informix_sync_queue`, writing deferred
//!    CEO decisions back to Informix via ODBC. Called by the 2-min cron and by
//!    the per-record sync API endpoint.
//!
//! 2. `refresh_review_queue` — pulls any new Informix `review_record` rows
//!    (status = 'P') that haven't been ingested into PG `status_reviews` yet.
//!    Called by the 5-min cron. Idempotent.

use std::sync::Arc;

use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::AppState;

// ── Helpers ──────────────────────────────────────────────────

fn col_str(batch: &TextRowSet, col: usize, row: usize) -> Option<String> {
    batch
        .at(col, row)
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
}

/// Extract a field from a sync-queue payload as a string.
/// Handles both JSON string and JSON integer values.
fn get_str(payload: &serde_json::Value, key: &str) -> Result<String, String> {
    if let Some(s) = payload[key].as_str() {
        return Ok(s.to_string());
    }
    if let Some(n) = payload[key].as_i64() {
        return Ok(n.to_string());
    }
    Err(format!("Missing or invalid '{key}' in sync payload"))
}

// ── Informix operation executor ───────────────────────────────

/// Execute one Informix ODBC write from a sync queue row.
/// All SQL is built from trusted enum-like `operation` values and
/// integer/short-string payload fields; no user-supplied strings reach Informix.
fn execute_informix_op(
    state: &AppState,
    operation: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let conn = state
        .env
        .connect(
            &state.config.dsn,
            &state.config.user,
            &state.config.password,
            ConnectionOptions::default(),
        )
        .map_err(|e| format!("Informix connect failed: {e}"))?;

    let sql: String = match operation {
        "update_pool_member_status" => {
            let part_no = get_str(payload, "part_no")?;
            let pool_no = get_str(payload, "pool_no")?;
            let new_status = get_str(payload, "new_status")?;
            // new_status is a numeric pool_member status code (1/2/5/6/7)
            if !new_status.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("Invalid new_status '{new_status}' — must be numeric"));
            }
            format!(
                "UPDATE pool_member SET status = {new_status} \
                 WHERE part_no = {part_no} AND pool_no = {pool_no}"
            )
        }
        "close_review_record" => {
            let part_no = get_str(payload, "part_no")?;
            let pool_no = get_str(payload, "pool_no")?;
            format!(
                "UPDATE review_record SET status = 'C' \
                 WHERE part_no = {part_no} AND pool_no = {pool_no}"
            )
        }
        "send_review_record" => {
            let part_no = get_str(payload, "part_no")?;
            let pool_no = get_str(payload, "pool_no")?;
            format!(
                "UPDATE review_record SET status = 'S' \
                 WHERE part_no = {part_no} AND pool_no = {pool_no}"
            )
        }
        "reopen_review_record" => {
            let part_no = get_str(payload, "part_no")?;
            let pool_no = get_str(payload, "pool_no")?;
            format!(
                "UPDATE review_record SET status = 'P' \
                 WHERE part_no = {part_no} AND pool_no = {pool_no}"
            )
        }
        op => return Err(format!("Unknown operation '{op}'")),
    };

    // Connection is dropped here (before any await), keeping it off async executor.
    conn.execute(&sql, ())
        .map_err(|e| format!("Informix execute failed: {e}"))?;
    conn.execute("COMMIT WORK", ()).ok();

    Ok(())
}

// ── Sync queue processor ──────────────────────────────────────

pub struct SyncProcessResult {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct QueueRow {
    id: Uuid,
    operation: String,
    payload: sqlx::types::Json<serde_json::Value>,
    attempts: i32,
}

const MAX_ATTEMPTS: i32 = 3;

/// Process pending rows in `informix_sync_queue`.
///
/// When `filter_part_key` is `Some("part_no_pool_no")`, only rows whose
/// payload matches that part are processed (used by the per-record sync API).
/// When `None`, all pending rows are processed (used by the cron).
pub async fn process_sync_queue(
    state: &AppState,
    pg: &sqlx::PgPool,
    filter_part_key: Option<&str>,
) -> SyncProcessResult {
    let rows: Vec<QueueRow> = match filter_part_key {
        Some(key) => {
            let (part_no, pool_no) = match key.split_once('_') {
                Some((a, b)) => (a.to_string(), b.to_string()),
                None => {
                    return SyncProcessResult {
                        processed: 0,
                        succeeded: 0,
                        failed: 0,
                        errors: vec!["Invalid part_key format".into()],
                    };
                }
            };
            sqlx::query_as::<_, QueueRow>(
                "SELECT id, operation, payload, attempts \
                 FROM informix_sync_queue \
                 WHERE status = 'pending' \
                   AND payload->>'part_no' = $1 \
                   AND payload->>'pool_no' = $2",
            )
            .bind(&part_no)
            .bind(&pool_no)
            .fetch_all(pg)
            .await
            .unwrap_or_default()
        }
        None => sqlx::query_as::<_, QueueRow>(
            "SELECT id, operation, payload, attempts \
             FROM informix_sync_queue WHERE status = 'pending' \
             ORDER BY created_at",
        )
        .fetch_all(pg)
        .await
        .unwrap_or_default(),
    };

    let mut result = SyncProcessResult {
        processed: rows.len(),
        succeeded: 0,
        failed: 0,
        errors: Vec::new(),
    };

    for row in rows {
        match execute_informix_op(state, &row.operation, &row.payload.0) {
            Ok(()) => {
                let _ = sqlx::query(
                    "UPDATE informix_sync_queue \
                     SET status = 'completed', completed_at = now() \
                     WHERE id = $1",
                )
                .bind(row.id)
                .execute(pg)
                .await;
                result.succeeded += 1;
                info!(
                    id = %row.id,
                    op = %row.operation,
                    "sync_queue: row completed"
                );
            }
            Err(e) => {
                let new_attempts = row.attempts + 1;
                let new_status = if new_attempts >= MAX_ATTEMPTS {
                    "failed"
                } else {
                    "pending"
                };
                let _ = sqlx::query(
                    "UPDATE informix_sync_queue \
                     SET attempts = $1, last_error = $2, status = $3 \
                     WHERE id = $4",
                )
                .bind(new_attempts)
                .bind(&e)
                .bind(new_status)
                .bind(row.id)
                .execute(pg)
                .await;
                result.failed += 1;
                result.errors.push(format!("{}: {}", row.operation, e));
                warn!(
                    id = %row.id,
                    op = %row.operation,
                    attempts = new_attempts,
                    error = %e,
                    "sync_queue: row failed"
                );
            }
        }
    }

    result
}

// ── Review queue refresh ──────────────────────────────────────

/// Pull new Informix `review_record` rows (status = 'P') into PG
/// `status_reviews`. Inserts only; existing PG records are never overwritten.
/// Returns the count of newly inserted rows.
pub async fn refresh_review_queue(state: &AppState, pg: &sqlx::PgPool) -> usize {
    let conn = match state.env.connect(
        &state.config.dsn,
        &state.config.user,
        &state.config.password,
        ConnectionOptions::default(),
    ) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "refresh_review_queue: Informix connect failed");
            return 0;
        }
    };

    let sql = "SELECT rr.part_no, rr.pool_no, rr.review_type \
               FROM review_record rr WHERE rr.status = 'P'";

    let mut records: Vec<(i32, i32, String)> = Vec::new();

    if let Ok(Some(mut stmt)) = conn.execute(sql, ()) {
        if let Ok(mut buf) = TextRowSet::for_cursor(500, &mut stmt, Some(64)) {
            if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                while let Ok(Some(batch)) = cursor.fetch() {
                    for row in 0..batch.num_rows() {
                        if let (Some(pn), Some(pl), Some(rt)) = (
                            col_str(&batch, 0, row).and_then(|s| s.parse::<i32>().ok()),
                            col_str(&batch, 1, row).and_then(|s| s.parse::<i32>().ok()),
                            col_str(&batch, 2, row),
                        ) {
                            records.push((pn, pl, rt));
                        }
                    }
                }
            }
        }
    }

    drop(conn); // release before awaiting

    let mut inserted = 0usize;
    for (part_no, pool_no, review_type) in records {
        // Guard: only known review types enter the queue
        if review_type != "excuse" && review_type != "disqualify" {
            warn!(
                part_no,
                pool_no,
                review_type = %review_type,
                "refresh_review_queue: skipping unknown review_type"
            );
            continue;
        }
        let part_key = format!("{part_no}_{pool_no}");
        match sqlx::query(
            "INSERT INTO status_reviews (part_no, pool_no, part_key, review_type, status) \
             VALUES ($1, $2, $3, $4, 'pending_admin') \
             ON CONFLICT (part_key) DO NOTHING",
        )
        .bind(part_no.to_string())
        .bind(pool_no.to_string())
        .bind(&part_key)
        .bind(&review_type)
        .execute(pg)
        .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                inserted += 1;
                info!(
                    part_key = %part_key,
                    review_type = %review_type,
                    "refresh_review_queue: inserted new record"
                );
            }
            Err(e) => {
                error!(error = %e, part_key = %part_key, "refresh_review_queue: insert failed");
            }
            _ => {}
        }
    }

    inserted
}

// ── Cron task launchers ───────────────────────────────────────

/// Spawn the 2-minute sync_informix_queue cron task.
pub fn spawn_sync_queue_cron(state: Arc<AppState>, pg: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(120));
        loop {
            interval.tick().await;
            let r = process_sync_queue(&state, &pg, None).await;
            if r.processed > 0 {
                info!(
                    processed = r.processed,
                    succeeded = r.succeeded,
                    failed = r.failed,
                    "sync_informix_queue cron tick"
                );
            }
        }
    });
}

/// Spawn the 5-minute refresh_review_queue cron task.
pub fn spawn_review_refresh_cron(state: Arc<AppState>, pg: sqlx::PgPool) {
    tokio::spawn(async move {
        // Skip the first immediate tick so the task doesn't race with startup
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(300));
        interval.tick().await; // consume the immediate first tick
        loop {
            interval.tick().await;
            let n = refresh_review_queue(&state, &pg).await;
            if n > 0 {
                info!(inserted = n, "refresh_review_queue cron tick");
            }
        }
    });
}
