use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::Json,
    Extension,
};
use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use tracing::{error, info};
use uuid::Uuid;

use crate::AppState;
use shared_types::{
    ActionResponse, AdminReviewQueue, AdminReviewRow, CeoDecideParams, CeoReviewQueue,
    CeoReviewRow, CeoReviewStateResponse, DecideResponse, ErrorResponse, PendingCountsResponse,
    ReviewDetail, ReviewHistoryEntry, ReviewHistoryResponse, SendToCeoParams, SetCeoStateParams,
    UserSession,
};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

fn api_err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

fn col_str(batch: &TextRowSet, col: usize, row: usize) -> Option<String> {
    batch
        .at(col, row)
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
}

/// Parse "part_no_pool_no" into (part_no, pool_no).
fn parse_part_key(key: &str) -> Option<(i32, i32)> {
    let (a, b) = key.split_once('_')?;
    let part_no: i32 = a.parse().ok()?;
    let pool_no: i32 = b.parse().ok()?;
    Some((part_no, pool_no))
}

// ── Admin queues ────────────────────────────────────────────

async fn admin_queue_impl(
    state: &Arc<AppState>,
    review_type: &str,
) -> ApiResult<AdminReviewQueue> {
    let conn = state
        .env
        .connect(
            &state.config.dsn,
            &state.config.user,
            &state.config.password,
            ConnectionOptions::default(),
        )
        .map_err(|e| {
            error!(error = %e, "AdminQueue: connect failed");
            api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}"))
        })?;

    let sql = format!(
        "SELECT rr.rr_id, rr.part_no, rr.pool_no, p.fname, p.lname, \
                rr.review_type, rr.status, rr.admin_notes, rr.submitted_date \
         FROM review_record rr \
         JOIN participant p ON p.part_no = rr.part_no \
         WHERE rr.status = 'P' AND rr.review_type = '{}' \
         ORDER BY rr.submitted_date, p.lname, p.fname",
        review_type.replace('\'', "''")
    );

    let mut stmt = conn
        .execute(&sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(4096))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt
        .bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor
        .fetch()
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))?
    {
        for row in 0..batch.num_rows() {
            let part_no: i32 = col_str(&batch, 1, row)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let pool_no: i32 = col_str(&batch, 2, row)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            rows.push(AdminReviewRow {
                rr_id: col_str(&batch, 0, row)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                part_no,
                pool_no,
                part_key: format!("{part_no}_{pool_no}"),
                fname: col_str(&batch, 3, row),
                lname: col_str(&batch, 4, row),
                review_type: col_str(&batch, 5, row).unwrap_or_default(),
                status: col_str(&batch, 6, row).unwrap_or_default(),
                admin_notes: col_str(&batch, 7, row),
                submitted_date: col_str(&batch, 8, row),
            });
        }
    }

    let count = rows.len();
    info!(review_type, count, "Admin review queue loaded");
    Ok(Json(AdminReviewQueue { rows, count }))
}

/// GET /api/reviews/excuse/admin
pub async fn admin_excuse_queue_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<AdminReviewQueue> {
    admin_queue_impl(&state, "excuse").await
}

/// GET /api/reviews/disqualify/admin
pub async fn admin_disqualify_queue_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<AdminReviewQueue> {
    admin_queue_impl(&state, "disqualify").await
}

// ── Review detail ───────────────────────────────────────────

/// GET /api/reviews/:part_key
pub async fn review_detail_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    AxumPath(part_key): AxumPath<String>,
) -> ApiResult<ReviewDetail> {
    let (part_no, pool_no) = parse_part_key(&part_key)
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "Invalid part_key format (expected part_no_pool_no)"))?;

    // odbc_api types are !Send — isolate all ODBC work in a block so nothing
    // non-Send is live across the .await below.
    let mut detail: ReviewDetail = {
        let conn = state
            .env
            .connect(
                &state.config.dsn,
                &state.config.user,
                &state.config.password,
                ConnectionOptions::default(),
            )
            .map_err(|e| {
                error!(error = %e, "ReviewDetail: connect failed");
                api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}"))
            })?;

        let sql = format!(
            "SELECT FIRST 1 \
                p.part_no, p.fname, p.lname, p.addr, p.city, p.state, p.zip, p.email, \
                p.gender, p.race_code, p.active, \
                po.div_code, po.ret_date, \
                pm.pm_id, pm.status, \
                rr.review_type, rr.status, rr.admin_notes, rr.submitted_date \
             FROM participant p \
             JOIN pool_member pm ON pm.part_no = p.part_no AND pm.pool_no = {pool_no} \
             JOIN pool po ON po.pool_no = {pool_no} \
             JOIN review_record rr ON rr.part_no = p.part_no AND rr.pool_no = {pool_no} \
             WHERE p.part_no = {part_no} \
             ORDER BY rr.submitted_date DESC"
        );

        let mut stmt = conn
            .execute(&sql, ())
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
            .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "Review record not found"))?;

        let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(4096))
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
        let mut cursor = stmt
            .bind_buffer(&mut buf)
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

        let batch = cursor
            .fetch()
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))?
            .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "Review record not found"))?;

        let row = 0;
        ReviewDetail {
            part_no,
            pool_no,
            part_key: part_key.clone(),
            fname: col_str(&batch, 1, row),
            lname: col_str(&batch, 2, row),
            addr: col_str(&batch, 3, row),
            city: col_str(&batch, 4, row),
            state_code: col_str(&batch, 5, row),
            zip: col_str(&batch, 6, row),
            email: col_str(&batch, 7, row),
            gender: col_str(&batch, 8, row),
            race_code: col_str(&batch, 9, row),
            active: col_str(&batch, 10, row),
            pool_div_code: col_str(&batch, 11, row),
            pool_ret_date: col_str(&batch, 12, row),
            pm_id: col_str(&batch, 13, row)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            member_status: col_str(&batch, 14, row)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            review_type: col_str(&batch, 15, row).unwrap_or_default(),
            ifx_status: col_str(&batch, 16, row).unwrap_or_default(),
            admin_notes: col_str(&batch, 17, row),
            submitted_date: col_str(&batch, 18, row),
            // PG fields — populated below
            pg_status: None,
            ceo_notes: None,
            decision: None,
            sent_to_ceo_at: None,
            decided_at: None,
        }
        // cursor, buf, conn all dropped here — no !Send types escape the block
    };

    // Overlay PG status_reviews data if present
    if let Some(pg) = &state.pg_pool {
        #[derive(sqlx::FromRow)]
        struct PgRow {
            status: String,
            admin_notes: Option<String>,
            ceo_notes: Option<String>,
            decision: Option<String>,
            sent_to_ceo_at: Option<chrono::DateTime<chrono::Utc>>,
            decided_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        if let Ok(Some(pg_row)) = sqlx::query_as::<_, PgRow>(
            "SELECT status, admin_notes, ceo_notes, decision, sent_to_ceo_at, decided_at \
             FROM status_reviews WHERE part_key = $1",
        )
        .bind(&part_key)
        .fetch_optional(pg)
        .await
        {
            // Prefer PG admin_notes if set (admin may have updated via send-to-ceo)
            if pg_row.admin_notes.is_some() {
                detail.admin_notes = pg_row.admin_notes;
            }
            detail.pg_status = Some(pg_row.status);
            detail.ceo_notes = pg_row.ceo_notes;
            detail.decision = pg_row.decision;
            detail.sent_to_ceo_at = pg_row.sent_to_ceo_at.map(|t| t.to_rfc3339());
            detail.decided_at = pg_row.decided_at.map(|t| t.to_rfc3339());
        }
    }

    info!(part_key = %part_key, "Review detail loaded");
    Ok(Json(detail))
}

// ── Send to CEO ─────────────────────────────────────────────

/// POST /api/reviews/send-to-ceo
pub async fn send_to_ceo_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<SendToCeoParams>,
) -> ApiResult<ActionResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let (part_no, pool_no) = parse_part_key(&params.part_key)
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "Invalid part_key"))?;

    // Get review_type from Informix — odbc_api types are !Send; isolate in block
    // so nothing non-Send is live across the .await points below.
    let review_type: String = {
        let conn = state
            .env
            .connect(
                &state.config.dsn,
                &state.config.user,
                &state.config.password,
                ConnectionOptions::default(),
            )
            .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

        let type_sql = format!(
            "SELECT FIRST 1 review_type FROM review_record \
             WHERE part_no = {part_no} AND pool_no = {pool_no} \
             ORDER BY submitted_date DESC"
        );
        let mut stmt = conn
            .execute(&type_sql, ())
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
            .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "No review record found for this participant/pool"))?;

        let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(64))
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
        let mut cursor = stmt
            .bind_buffer(&mut buf)
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

        cursor
            .fetch()
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))?
            .and_then(|batch| col_str(&batch, 0, 0))
            .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "Review record not found"))?
        // conn, cursor, buf all dropped here
    };

    // Check if already completed in PG
    let existing_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM status_reviews WHERE part_key = $1",
    )
    .bind(&params.part_key)
    .fetch_optional(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query failed: {e}")))?;

    if existing_status.as_deref() == Some("completed") {
        return Err(api_err(StatusCode::CONFLICT, "Review is already completed"));
    }

    // Upsert status_reviews
    let review_id: Uuid = if existing_status.is_some() {
        sqlx::query_scalar(
            "UPDATE status_reviews \
             SET status = 'pending_ceo', admin_notes = $1, sent_to_ceo_at = now(), updated_at = now() \
             WHERE part_key = $2 \
             RETURNING id",
        )
        .bind(&params.admin_notes)
        .bind(&params.part_key)
        .fetch_one(pg)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG update failed: {e}")))?
    } else {
        sqlx::query_scalar(
            "INSERT INTO status_reviews \
             (part_no, pool_no, part_key, review_type, status, admin_notes, sent_to_ceo_at) \
             VALUES ($1, $2, $3, $4, 'pending_ceo', $5, now()) \
             RETURNING id",
        )
        .bind(part_no.to_string())
        .bind(pool_no.to_string())
        .bind(&params.part_key)
        .bind(&review_type)
        .bind(&params.admin_notes)
        .fetch_one(pg)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG insert failed: {e}")))?
    };

    // Insert review_history
    sqlx::query(
        "INSERT INTO review_history \
         (status_review_id, part_no, review_type, action, actor_sub, actor_email, notes) \
         VALUES ($1, $2, $3, 'sent_to_ceo', $4, $5, $6)",
    )
    .bind(review_id)
    .bind(part_no.to_string())
    .bind(&review_type)
    .bind(&user.sub)
    .bind(&user.email)
    .bind(&params.admin_notes)
    .execute(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("History insert failed: {e}")))?;

    // Update Informix review_record (new connection — odbc_api types are !Send)
    {
        let conn = state
            .env
            .connect(
                &state.config.dsn,
                &state.config.user,
                &state.config.password,
                ConnectionOptions::default(),
            )
            .map_err(|e| {
                error!(error = %e, part_no, pool_no, "Failed to connect for Informix update");
                api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Informix connect failed: {e}"))
            })?;
        conn.execute(
            &format!(
                "UPDATE review_record SET status = 'S' \
                 WHERE part_no = {part_no} AND pool_no = {pool_no}"
            ),
            (),
        )
        .map_err(|e| {
            error!(error = %e, part_no, pool_no, "Failed to update Informix review_record status");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Informix update failed: {e}"))
        })?;
    }

    info!(part_key = %params.part_key, actor = %user.sub, "Sent review to CEO");
    Ok(Json(ActionResponse {
        ok: true,
        message: format!("Review {} sent to CEO", params.part_key),
    }))
}

// ── CEO queue ────────────────────────────────────────────────

/// GET /api/reviews/ceo  (requires ceo-review group)
pub async fn ceo_queue_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
) -> ApiResult<CeoReviewQueue> {
    if !user.groups.iter().any(|g| g == "ceo-review") {
        return Err(api_err(StatusCode::FORBIDDEN, "ceo-review group required"));
    }

    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    // Check maintenance mode
    let ceo_state: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_config WHERE key = 'ceo_review_state'",
    )
    .fetch_optional(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Config query failed: {e}")))?;

    if ceo_state.as_deref() == Some("maintenance") {
        return Ok(Json(CeoReviewQueue {
            rows: Vec::new(),
            count: 0,
            maintenance: true,
        }));
    }

    #[derive(sqlx::FromRow)]
    struct PgQueueRow {
        id: Uuid,
        part_no: String,
        pool_no: String,
        part_key: String,
        review_type: String,
        admin_notes: Option<String>,
        sent_to_ceo_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let pg_rows = sqlx::query_as::<_, PgQueueRow>(
        "SELECT id, part_no, pool_no, part_key, review_type, admin_notes, sent_to_ceo_at \
         FROM status_reviews WHERE status = 'pending_ceo' \
         ORDER BY sent_to_ceo_at",
    )
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query failed: {e}")))?;

    if pg_rows.is_empty() {
        return Ok(Json(CeoReviewQueue {
            rows: Vec::new(),
            count: 0,
            maintenance: false,
        }));
    }

    // Fetch participant names from Informix
    let part_nos: Vec<i32> = pg_rows
        .iter()
        .filter_map(|r| r.part_no.parse::<i32>().ok())
        .collect();
    let in_clause = part_nos
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut name_map: std::collections::HashMap<i32, (Option<String>, Option<String>)> =
        std::collections::HashMap::new();

    if let Ok(conn) = state.env.connect(
        &state.config.dsn,
        &state.config.user,
        &state.config.password,
        ConnectionOptions::default(),
    ) {
        let name_sql = format!(
            "SELECT part_no, fname, lname FROM participant WHERE part_no IN ({in_clause})"
        );
        if let Ok(Some(mut nstmt)) = conn.execute(&name_sql, ()) {
            if let Ok(mut nbuf) = TextRowSet::for_cursor(100, &mut nstmt, Some(256)) {
                if let Ok(mut ncursor) = nstmt.bind_buffer(&mut nbuf) {
                    while let Ok(Some(nbatch)) = ncursor.fetch() {
                        for nrow in 0..nbatch.num_rows() {
                            if let Some(pno) = col_str(&nbatch, 0, nrow)
                                .and_then(|s| s.parse::<i32>().ok())
                            {
                                name_map.insert(
                                    pno,
                                    (col_str(&nbatch, 1, nrow), col_str(&nbatch, 2, nrow)),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let rows: Vec<CeoReviewRow> = pg_rows
        .into_iter()
        .map(|r| {
            let pno: i32 = r.part_no.parse().unwrap_or(0);
            let (fname, lname) = name_map.get(&pno).cloned().unwrap_or((None, None));
            CeoReviewRow {
                id: r.id.to_string(),
                part_no: r.part_no,
                pool_no: r.pool_no,
                part_key: r.part_key,
                fname,
                lname,
                review_type: r.review_type,
                admin_notes: r.admin_notes,
                sent_to_ceo_at: r.sent_to_ceo_at.map(|t| t.to_rfc3339()),
            }
        })
        .collect();

    let count = rows.len();
    info!(count, "CEO queue loaded");
    Ok(Json(CeoReviewQueue {
        rows,
        count,
        maintenance: false,
    }))
}

// ── CEO decision ─────────────────────────────────────────────

/// POST /api/reviews/ceo/decide  (requires ceo-review group)
/// Async write pattern: all writes go to PostgreSQL; Informix is updated by sync cron.
pub async fn ceo_decide_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<CeoDecideParams>,
) -> ApiResult<DecideResponse> {
    if !user.groups.iter().any(|g| g == "ceo-review") {
        return Err(api_err(StatusCode::FORBIDDEN, "ceo-review group required"));
    }

    const VALID_ACTIONS: &[&str] = &[
        "requalify",
        "disqualify",
        "permanent_excuse",
        "temporary_excuse",
        "send_back",
    ];
    if !VALID_ACTIONS.contains(&params.action.as_str()) {
        return Err(api_err(
            StatusCode::BAD_REQUEST,
            format!("Invalid action: {}", params.action),
        ));
    }

    let (part_no, pool_no) = parse_part_key(&params.part_key)
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "Invalid part_key"))?;

    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    // Open transaction with row lock for idempotency
    let mut tx = pg.begin().await.map_err(|e| {
        api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Transaction begin failed: {e}"))
    })?;

    #[derive(sqlx::FromRow)]
    struct ReviewRow {
        id: Uuid,
        status: String,
        decision: Option<String>,
        review_type: String,
    }

    let row = sqlx::query_as::<_, ReviewRow>(
        "SELECT id, status, decision, review_type \
         FROM status_reviews WHERE part_key = $1 FOR UPDATE",
    )
    .bind(&params.part_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    let Some(review) = row else {
        tx.rollback().await.ok();
        return Err(api_err(StatusCode::NOT_FOUND, "Review not found in CEO queue"));
    };

    // Idempotency: already acted on
    if review.status == "completed" || review.status == "sent_back" {
        tx.commit().await.ok();
        return Ok(Json(DecideResponse {
            ok: true,
            message: "Decision already recorded".to_string(),
            was_duplicate: true,
            status: review.status,
            decision: review.decision,
        }));
    }

    if review.status != "pending_ceo" {
        tx.rollback().await.ok();
        return Err(api_err(
            StatusCode::CONFLICT,
            format!("Review is in status '{}', expected 'pending_ceo'", review.status),
        ));
    }

    let (new_status, decision_val): (&str, Option<String>) = if params.action == "send_back" {
        ("sent_back", None)
    } else {
        ("completed", Some(params.action.clone()))
    };

    // Map action → pool_member status code
    let pm_status: Option<i32> = match params.action.as_str() {
        "requalify" => Some(2),
        "disqualify" => Some(6),
        "permanent_excuse" => Some(5),
        "temporary_excuse" => Some(7),
        _ => None, // send_back
    };

    // Update status_reviews (guard: only if still pending_ceo)
    let affected = sqlx::query(
        "UPDATE status_reviews \
         SET status = $1, decision = $2, ceo_notes = $3, \
             decided_at = now(), decided_by = $4, updated_at = now() \
         WHERE id = $5 AND status = 'pending_ceo'",
    )
    .bind(new_status)
    .bind(&decision_val)
    .bind(&params.notes)
    .bind(&user.sub)
    .bind(review.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?
    .rows_affected();

    if affected == 0 {
        // Race: someone else acted concurrently — re-read and return
        tx.rollback().await.ok();
        let current = sqlx::query_as::<_, ReviewRow>(
            "SELECT id, status, decision, review_type FROM status_reviews WHERE part_key = $1",
        )
        .bind(&params.part_key)
        .fetch_optional(pg)
        .await
        .ok()
        .flatten();
        return Ok(Json(DecideResponse {
            ok: true,
            message: "Decision already recorded (concurrent)".to_string(),
            was_duplicate: true,
            status: current.as_ref().map(|r| r.status.clone()).unwrap_or_default(),
            decision: current.and_then(|r| r.decision),
        }));
    }

    // Insert review_history
    sqlx::query(
        "INSERT INTO review_history \
         (status_review_id, part_no, review_type, action, actor_sub, actor_email, notes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(review.id)
    .bind(part_no.to_string())
    .bind(&review.review_type)
    .bind(&params.action)
    .bind(&user.sub)
    .bind(&user.email)
    .bind(&params.notes)
    .execute(&mut *tx)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("History insert failed: {e}")))?;

    // Enqueue Informix sync operations
    if let Some(status_code) = pm_status {
        // Final decision: update pool_member status + close review_record
        sqlx::query(
            "INSERT INTO informix_sync_queue (operation, payload) VALUES ($1, $2)",
        )
        .bind("update_pool_member_status")
        .bind(serde_json::json!({
            "part_no": part_no,
            "pool_no": pool_no,
            "new_status": status_code
        }))
        .execute(&mut *tx)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Sync queue insert failed: {e}")))?;

        sqlx::query(
            "INSERT INTO informix_sync_queue (operation, payload) VALUES ($1, $2)",
        )
        .bind("close_review_record")
        .bind(serde_json::json!({
            "part_no": part_no,
            "pool_no": pool_no
        }))
        .execute(&mut *tx)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Sync queue insert failed: {e}")))?;
    } else {
        // send_back: reopen in Informix
        sqlx::query(
            "INSERT INTO informix_sync_queue (operation, payload) VALUES ($1, $2)",
        )
        .bind("reopen_review_record")
        .bind(serde_json::json!({
            "part_no": part_no,
            "pool_no": pool_no
        }))
        .execute(&mut *tx)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Sync queue insert failed: {e}")))?;
    }

    tx.commit().await.map_err(|e| {
        api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Commit failed: {e}"))
    })?;

    crate::audit::admin_access(
        &user.sub,
        &format!("/api/reviews/ceo/decide/{}", params.part_key),
        Some(&format!("action={}", params.action)),
    );

    info!(
        part_key = %params.part_key,
        action = %params.action,
        actor = %user.sub,
        "CEO decision recorded"
    );

    Ok(Json(DecideResponse {
        ok: true,
        message: format!("Decision '{}' recorded for {}", params.action, params.part_key),
        was_duplicate: false,
        status: new_status.to_string(),
        decision: decision_val,
    }))
}

// ── Review history ───────────────────────────────────────────

/// GET /api/reviews/records/:part_no
pub async fn review_history_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    AxumPath(part_no): AxumPath<String>,
) -> ApiResult<ReviewHistoryResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    #[derive(sqlx::FromRow)]
    struct HistRow {
        id: Uuid,
        part_no: String,
        review_type: String,
        action: String,
        actor_email: Option<String>,
        notes: Option<String>,
        acted_at: chrono::DateTime<chrono::Utc>,
    }

    let rows = sqlx::query_as::<_, HistRow>(
        "SELECT id, part_no, review_type, action, actor_email, notes, acted_at \
         FROM review_history WHERE part_no = $1 ORDER BY acted_at DESC",
    )
    .bind(&part_no)
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    let entries: Vec<ReviewHistoryEntry> = rows
        .into_iter()
        .map(|r| ReviewHistoryEntry {
            id: r.id.to_string(),
            part_no: r.part_no,
            review_type: r.review_type,
            action: r.action,
            actor_email: r.actor_email,
            notes: r.notes,
            acted_at: r.acted_at.to_rfc3339(),
        })
        .collect();

    let count = entries.len();
    Ok(Json(ReviewHistoryResponse { entries, count }))
}

// ── Pending counts ───────────────────────────────────────────

/// GET /api/reviews/pending
pub async fn pending_counts_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<PendingCountsResponse> {
    let conn = state
        .env
        .connect(
            &state.config.dsn,
            &state.config.user,
            &state.config.password,
            ConnectionOptions::default(),
        )
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    let excuse_pending = {
        let mut stmt = conn
            .execute(
                "SELECT COUNT(*) FROM review_record WHERE status = 'P' AND review_type = 'excuse'",
                (),
            )
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
            .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;
        let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(64))
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer: {e}")))?;
        let mut cursor = stmt
            .bind_buffer(&mut buf)
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind: {e}")))?;
        cursor
            .fetch()
            .ok()
            .flatten()
            .and_then(|b| col_str(&b, 0, 0))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    let disqualify_pending = {
        let mut stmt = conn
            .execute(
                "SELECT COUNT(*) FROM review_record WHERE status = 'P' AND review_type = 'disqualify'",
                (),
            )
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
            .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;
        let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(64))
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer: {e}")))?;
        let mut cursor = stmt
            .bind_buffer(&mut buf)
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind: {e}")))?;
        cursor
            .fetch()
            .ok()
            .flatten()
            .and_then(|b| col_str(&b, 0, 0))
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };

    drop(conn);

    let ceo_queue = if let Some(pg) = &state.pg_pool {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM status_reviews WHERE status = 'pending_ceo'",
        )
        .fetch_one(pg)
        .await
        .unwrap_or(0)
    } else {
        0
    };

    Ok(Json(PendingCountsResponse {
        excuse_pending,
        disqualify_pending,
        ceo_queue,
    }))
}

// ── CEO review state ─────────────────────────────────────────

/// GET /api/reviews/ceo-state
pub async fn get_ceo_state_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<CeoReviewStateResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let val: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_config WHERE key = 'ceo_review_state'")
            .fetch_optional(pg)
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    Ok(Json(CeoReviewStateResponse {
        state: val.unwrap_or_else(|| "live".to_string()),
    }))
}

/// POST /api/reviews/ceo-state
pub async fn set_ceo_state_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<SetCeoStateParams>,
) -> ApiResult<CeoReviewStateResponse> {
    if params.state != "live" && params.state != "maintenance" {
        return Err(api_err(StatusCode::BAD_REQUEST, "State must be 'live' or 'maintenance'"));
    }

    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    sqlx::query(
        "UPDATE app_config SET value = $1, updated_at = now(), updated_by = $2 \
         WHERE key = 'ceo_review_state'",
    )
    .bind(&params.state)
    .bind(&user.sub)
    .execute(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?;

    info!(state = %params.state, actor = %user.sub, "CEO review state toggled");
    Ok(Json(CeoReviewStateResponse { state: params.state }))
}
