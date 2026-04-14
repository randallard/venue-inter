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
    CeoReviewRow, CeoReviewStateResponse, DecideResponse, DocumentSnapshot, ErrorResponse,
    ParticipantCheck, PendingCountsResponse, PoolMemberSnapshot, ReviewDetail, ReviewHistoryEntry,
    ReviewHistoryResponse, ReviewRecordSnapshot, ReviewReport, ReviewReportRow, SendToCeoParams,
    SetCeoStateParams, StatusReviewSnapshot, SyncQueueSnapshot, UnifiedReviewQueue, UnifiedReviewRow,
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
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    #[derive(sqlx::FromRow)]
    struct Counts {
        excuse_pending: i64,
        disqualify_pending: i64,
        ceo_queue: i64,
    }

    let counts = sqlx::query_as::<_, Counts>(
        "SELECT \
            COUNT(*) FILTER (WHERE status = 'pending_admin' AND review_type = 'excuse')     AS excuse_pending, \
            COUNT(*) FILTER (WHERE status = 'pending_admin' AND review_type = 'disqualify') AS disqualify_pending, \
            COUNT(*) FILTER (WHERE status = 'pending_ceo')                                  AS ceo_queue \
         FROM status_reviews",
    )
    .fetch_one(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    Ok(Json(PendingCountsResponse {
        excuse_pending: counts.excuse_pending,
        disqualify_pending: counts.disqualify_pending,
        ceo_queue: counts.ceo_queue,
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

// ── Sync status ──────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct SyncStatusRow {
    pub part_no: String,
    pub pool_no: String,
    pub part_key: String,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub review_type: String,
    /// Current status in Informix review_record ('P' = pending, other = closed/processed).
    /// None means no review_record row was found for this part_key.
    pub ifx_status: Option<String>,
    /// Status in PostgreSQL status_reviews. None means the record hasn't been
    /// picked up by admin yet (lives only in Informix).
    pub pg_status: Option<String>,
    pub pg_decision: Option<String>,
    /// Pending sync_queue operations for this record.
    pub sync_pending: i64,
    /// Failed sync_queue operations for this record.
    pub sync_failed: i64,
    /// Error messages from failed sync operations.
    pub sync_errors: Vec<String>,
    /// ok | active | syncing | warning | error | unprocessed
    pub health: String,
    pub health_reason: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SyncStatusResponse {
    pub rows: Vec<SyncStatusRow>,
    pub total: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub syncing_count: usize,
    pub unprocessed_count: usize,
}

fn sync_health(
    ifx_status: Option<&str>,
    pg_status: Option<&str>,
    sync_pending: i64,
    sync_failed: i64,
) -> (&'static str, Option<&'static str>) {
    if sync_failed > 0 {
        return ("error", Some("Sync operation failed — Informix not updated"));
    }
    match pg_status {
        None => match ifx_status {
            Some("P") => ("unprocessed", None),
            Some(_) => ("ok", None), // closed in Informix, never needed PG processing
            None => ("ok", None),
        },
        Some(pg) => {
            let ifx_open = matches!(ifx_status, Some("P") | Some("S"));
            let ifx_missing = ifx_status.is_none();
            match pg {
                "completed" | "sent_back" => {
                    if sync_pending > 0 {
                        ("syncing", None)
                    } else if ifx_missing {
                        ("error", Some("Orphan — record in PG but not found in Informix"))
                    } else if ifx_open {
                        ("error", Some("Stale — sync complete but Informix review_record still open"))
                    } else {
                        ("ok", None)
                    }
                }
                "pending_admin" | "pending_ceo" => {
                    if ifx_missing {
                        ("error", Some("Orphan — record in PG but not found in Informix"))
                    } else if !ifx_open {
                        ("warning", Some("Informix review_record closed but PG workflow still active"))
                    } else {
                        ("active", None)
                    }
                }
                _ => ("ok", None),
            }
        }
    }
}

/// GET /api/reviews/sync-status
///
/// Cross-system view of all review records: Informix review_record vs
/// PostgreSQL status_reviews vs informix_sync_queue. Shows every record
/// that is either pending in Informix or has been touched by this app
/// in PG, along with its sync health.
pub async fn sync_status_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<SyncStatusResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    // ── 1. PostgreSQL: all status_reviews rows ──────────────
    #[derive(sqlx::FromRow)]
    struct PgRow {
        part_no: String,
        pool_no: String,
        part_key: String,
        review_type: String,
        status: String,
        decision: Option<String>,
    }

    let pg_rows = sqlx::query_as::<_, PgRow>(
        "SELECT part_no, pool_no, part_key, review_type, status, decision \
         FROM status_reviews ORDER BY created_at DESC",
    )
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query failed: {e}")))?;

    // ── 2. PostgreSQL: non-completed sync_queue entries ─────
    #[derive(sqlx::FromRow)]
    struct SyncRow {
        part_no: Option<String>,
        pool_no: Option<String>,
        status: String,
        last_error: Option<String>,
    }

    let sync_rows = sqlx::query_as::<_, SyncRow>(
        "SELECT payload->>'part_no' AS part_no, \
                payload->>'pool_no' AS pool_no, \
                status, last_error \
         FROM informix_sync_queue \
         WHERE status IN ('pending', 'failed')",
    )
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Sync queue query failed: {e}")))?;

    // Index sync rows by part_key
    let mut sync_map: std::collections::HashMap<String, (i64, i64, Vec<String>)> =
        std::collections::HashMap::new();
    for sr in sync_rows {
        if let (Some(pno), Some(plno)) = (sr.part_no, sr.pool_no) {
            let key = format!("{pno}_{plno}");
            let entry = sync_map.entry(key).or_insert((0, 0, Vec::new()));
            if sr.status == "pending" {
                entry.0 += 1;
            } else if sr.status == "failed" {
                entry.1 += 1;
                if let Some(e) = sr.last_error {
                    entry.2.push(e);
                }
            }
        }
    }

    // ── 3. Informix: pending records + all PG-referenced ones ──
    // Build IN clause from PG part_nos so we can check their current Informix status
    let pg_part_nos: Vec<i32> = pg_rows
        .iter()
        .filter_map(|r| r.part_no.parse::<i32>().ok())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Map of part_key → (fname, lname, ifx_status, review_type)
    let mut ifx_map: std::collections::HashMap<String, (Option<String>, Option<String>, String, String)> =
        std::collections::HashMap::new();

    if let Ok(conn) = state.env.connect(
        &state.config.dsn,
        &state.config.user,
        &state.config.password,
        ConnectionOptions::default(),
    ) {
        // All currently pending records
        let pending_sql = "SELECT rr.part_no, rr.pool_no, rr.review_type, rr.status, \
                           p.fname, p.lname \
                           FROM review_record rr \
                           JOIN participant p ON p.part_no = rr.part_no \
                           WHERE rr.status = 'P'";
        if let Ok(Some(mut stmt)) = conn.execute(pending_sql, ()) {
            if let Ok(mut buf) = TextRowSet::for_cursor(500, &mut stmt, Some(512)) {
                if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                    while let Ok(Some(batch)) = cursor.fetch() {
                        for row in 0..batch.num_rows() {
                            if let (Some(pno), Some(plno), Some(review_type), Some(status)) = (
                                col_str(&batch, 0, row),
                                col_str(&batch, 1, row),
                                col_str(&batch, 2, row),
                                col_str(&batch, 3, row),
                            ) {
                                let key = format!("{pno}_{plno}");
                                ifx_map.insert(
                                    key,
                                    (col_str(&batch, 4, row), col_str(&batch, 5, row), status, review_type),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Records referenced by PG rows but not already fetched (i.e. already closed)
        if !pg_part_nos.is_empty() {
            let in_clause = pg_part_nos
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let closed_sql = format!(
                "SELECT rr.part_no, rr.pool_no, rr.review_type, rr.status, p.fname, p.lname \
                 FROM review_record rr \
                 JOIN participant p ON p.part_no = rr.part_no \
                 WHERE rr.status != 'P' AND rr.part_no IN ({in_clause})"
            );
            if let Ok(Some(mut stmt)) = conn.execute(&closed_sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(500, &mut stmt, Some(512)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        while let Ok(Some(batch)) = cursor.fetch() {
                            for row in 0..batch.num_rows() {
                                if let (Some(pno), Some(plno), Some(review_type), Some(status)) = (
                                    col_str(&batch, 0, row),
                                    col_str(&batch, 1, row),
                                    col_str(&batch, 2, row),
                                    col_str(&batch, 3, row),
                                ) {
                                    let key = format!("{pno}_{plno}");
                                    // Don't overwrite 'P' entries already found
                                    ifx_map.entry(key).or_insert((
                                        col_str(&batch, 4, row),
                                        col_str(&batch, 5, row),
                                        status,
                                        review_type,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── 4. Merge and compute health ──────────────────────────

    // Start with all PG rows
    let pg_keys: std::collections::HashSet<String> =
        pg_rows.iter().map(|r| r.part_key.clone()).collect();

    let mut rows: Vec<SyncStatusRow> = pg_rows
        .into_iter()
        .map(|pg| {
            let (sync_pending, sync_failed, sync_errors) = sync_map
                .get(&pg.part_key)
                .cloned()
                .unwrap_or((0, 0, Vec::new()));
            let (fname, lname, ifx_status) = ifx_map
                .get(&pg.part_key)
                .cloned()
                .map(|(f, l, s, _rt)| (f, l, Some(s)))
                .unwrap_or((None, None, None));
            let (health, reason) = sync_health(
                ifx_status.as_deref(),
                Some(&pg.status),
                sync_pending,
                sync_failed,
            );
            SyncStatusRow {
                part_no: pg.part_no,
                pool_no: pg.pool_no,
                part_key: pg.part_key,
                fname,
                lname,
                review_type: pg.review_type,
                ifx_status,
                pg_status: Some(pg.status),
                pg_decision: pg.decision,
                sync_pending,
                sync_failed,
                sync_errors,
                health: health.to_string(),
                health_reason: reason.map(|s| s.to_string()),
            }
        })
        .collect();

    // Append Informix-only records (unprocessed — in Informix but not in PG)
    for (key, (fname, lname, ifx_status, review_type)) in &ifx_map {
        if pg_keys.contains(key) || ifx_status != "P" {
            continue; // already covered above, or closed record we don't care about
        }
        let (part_no, pool_no) = match key.split_once('_') {
            Some(p) => (p.0.to_string(), p.1.to_string()),
            None => continue,
        };
        rows.push(SyncStatusRow {
            part_no,
            pool_no,
            part_key: key.clone(),
            fname: fname.clone(),
            lname: lname.clone(),
            review_type: review_type.clone(),
            ifx_status: Some(ifx_status.clone()),
            pg_status: None,
            pg_decision: None,
            sync_pending: 0,
            sync_failed: 0,
            sync_errors: Vec::new(),
            health: "unprocessed".to_string(),
            health_reason: None,
        });
    }

    let total = rows.len();
    let error_count = rows.iter().filter(|r| r.health == "error").count();
    let warning_count = rows.iter().filter(|r| r.health == "warning").count();
    let syncing_count = rows.iter().filter(|r| r.health == "syncing").count();
    let unprocessed_count = rows.iter().filter(|r| r.health == "unprocessed").count();

    // Sort: errors first, then warnings, syncing, unprocessed, active, ok
    let health_order = |h: &str| match h {
        "error" => 0,
        "warning" => 1,
        "syncing" => 2,
        "unprocessed" => 3,
        "active" => 4,
        _ => 5,
    };
    rows.sort_by_key(|r| health_order(&r.health));

    Ok(Json(SyncStatusResponse {
        rows,
        total,
        error_count,
        warning_count,
        syncing_count,
        unprocessed_count,
    }))
}

// ── Per-record sync trigger ───────────────────────────────────

#[derive(serde::Serialize)]
pub struct SyncOneResponse {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// POST /api/reviews/sync-status/sync/:part_key
///
/// Immediately processes all pending `informix_sync_queue` rows for the
/// given `part_key`. Returns a summary of what was attempted and any errors.
pub async fn sync_one_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    AxumPath(part_key): AxumPath<String>,
) -> ApiResult<SyncOneResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let r = crate::sync::process_sync_queue(&state, pg, Some(&part_key)).await;

    Ok(Json(SyncOneResponse {
        processed: r.processed,
        succeeded: r.succeeded,
        failed: r.failed,
        errors: r.errors,
    }))
}

// ── Record lookup ─────────────────────────────────────────────

/// GET /api/reviews/sync-status/lookup/:query
///
/// Look up cross-system status for any participant, whether or not they
/// appear in the main sync-status listing. Accepts either:
///   • a part_key  (`part_no_pool_no`, e.g. "12345_678") — returns that record
///   • a part_no   (e.g. "12345") — returns all review records for that participant
///
/// Unlike the main listing, this includes ALL Informix review_record statuses
/// (not only 'P'), giving full visibility into closed / processed records.
pub async fn lookup_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    AxumPath(query): AxumPath<String>,
) -> ApiResult<SyncStatusResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    // Determine whether the query is a part_key or a bare part_no.
    let (part_no_str, pool_no_filter): (String, Option<String>) =
        if let Some((pn, pl)) = query.split_once('_') {
            (pn.to_string(), Some(pl.to_string()))
        } else {
            (query.clone(), None)
        };

    // ── PG: fetch matching status_reviews rows ───────────────
    #[derive(sqlx::FromRow)]
    struct PgRow {
        part_no: String,
        pool_no: String,
        part_key: String,
        review_type: String,
        status: String,
        decision: Option<String>,
    }

    let pg_rows: Vec<PgRow> = match &pool_no_filter {
        Some(pl) => {
            let key = format!("{part_no_str}_{pl}");
            sqlx::query_as::<_, PgRow>(
                "SELECT part_no, pool_no, part_key, review_type, status, decision \
                 FROM status_reviews WHERE part_key = $1",
            )
            .bind(&key)
            .fetch_all(pg)
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query: {e}")))?
        }
        None => {
            sqlx::query_as::<_, PgRow>(
                "SELECT part_no, pool_no, part_key, review_type, status, decision \
                 FROM status_reviews WHERE part_no = $1 ORDER BY created_at DESC",
            )
            .bind(&part_no_str)
            .fetch_all(pg)
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query: {e}")))?
        }
    };

    // ── PG: sync queue state for matched records ─────────────
    #[derive(sqlx::FromRow)]
    struct SyncRow {
        part_no: Option<String>,
        pool_no: Option<String>,
        status: String,
        last_error: Option<String>,
    }

    let pg_part_keys: Vec<String> = pg_rows.iter().map(|r| r.part_key.clone()).collect();

    let sync_rows: Vec<SyncRow> = sqlx::query_as::<_, SyncRow>(
        "SELECT payload->>'part_no' AS part_no, \
                payload->>'pool_no' AS pool_no, \
                status, last_error \
         FROM informix_sync_queue \
         WHERE status IN ('pending', 'failed') \
           AND payload->>'part_no' = $1",
    )
    .bind(&part_no_str)
    .fetch_all(pg)
    .await
    .unwrap_or_default();

    let mut sync_map: std::collections::HashMap<String, (i64, i64, Vec<String>)> =
        std::collections::HashMap::new();
    for sr in sync_rows {
        if let (Some(pno), Some(plno)) = (sr.part_no, sr.pool_no) {
            let key = format!("{pno}_{plno}");
            let entry = sync_map.entry(key).or_insert((0, 0, Vec::new()));
            if sr.status == "pending" {
                entry.0 += 1;
            } else if sr.status == "failed" {
                entry.1 += 1;
                if let Some(e) = sr.last_error {
                    entry.2.push(e);
                }
            }
        }
    }

    // ── Informix: ALL review_record rows for this part_no ───
    // (all statuses — gives full history for the lookup view)
    let mut ifx_map: std::collections::HashMap<String, (Option<String>, Option<String>, String, String)> =
        std::collections::HashMap::new();

    let part_no_int: Result<i32, _> = part_no_str.parse();
    if let Ok(pn) = part_no_int {
        if let Ok(conn) = state.env.connect(
            &state.config.dsn,
            &state.config.user,
            &state.config.password,
            ConnectionOptions::default(),
        ) {
            let sql = match &pool_no_filter {
                Some(pl) => format!(
                    "SELECT rr.part_no, rr.pool_no, rr.review_type, rr.status, \
                            p.fname, p.lname \
                     FROM review_record rr \
                     JOIN participant p ON p.part_no = rr.part_no \
                     WHERE rr.part_no = {pn} AND rr.pool_no = {pl}"
                ),
                None => format!(
                    "SELECT rr.part_no, rr.pool_no, rr.review_type, rr.status, \
                            p.fname, p.lname \
                     FROM review_record rr \
                     JOIN participant p ON p.part_no = rr.part_no \
                     WHERE rr.part_no = {pn}"
                ),
            };

            if let Ok(Some(mut stmt)) = conn.execute(&sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(100, &mut stmt, Some(512)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        while let Ok(Some(batch)) = cursor.fetch() {
                            for row in 0..batch.num_rows() {
                                if let (Some(part), Some(pool), Some(rt), Some(status)) = (
                                    col_str(&batch, 0, row),
                                    col_str(&batch, 1, row),
                                    col_str(&batch, 2, row),
                                    col_str(&batch, 3, row),
                                ) {
                                    let key = format!("{part}_{pool}");
                                    ifx_map.insert(
                                        key,
                                        (
                                            col_str(&batch, 4, row),
                                            col_str(&batch, 5, row),
                                            status,
                                            rt,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Merge ────────────────────────────────────────────────
    let pg_keys: std::collections::HashSet<String> =
        pg_rows.iter().map(|r| r.part_key.clone()).collect();

    let mut rows: Vec<SyncStatusRow> = pg_rows
        .into_iter()
        .map(|pg| {
            let (sync_pending, sync_failed, sync_errors) = sync_map
                .get(&pg.part_key)
                .cloned()
                .unwrap_or((0, 0, Vec::new()));
            let (fname, lname, ifx_status) = ifx_map
                .get(&pg.part_key)
                .cloned()
                .map(|(f, l, s, _rt)| (f, l, Some(s)))
                .unwrap_or((None, None, None));
            let (health, reason) = sync_health(
                ifx_status.as_deref(),
                Some(&pg.status),
                sync_pending,
                sync_failed,
            );
            SyncStatusRow {
                part_no: pg.part_no,
                pool_no: pg.pool_no,
                part_key: pg.part_key,
                fname,
                lname,
                review_type: pg.review_type,
                ifx_status,
                pg_status: Some(pg.status),
                pg_decision: pg.decision,
                sync_pending,
                sync_failed,
                sync_errors,
                health: health.to_string(),
                health_reason: reason.map(|s| s.to_string()),
            }
        })
        .collect();

    // Add Informix-only rows not found in PG
    for (key, (fname, lname, ifx_status, review_type)) in &ifx_map {
        if pg_keys.contains(key) {
            continue;
        }
        let (part_no, pool_no) = match key.split_once('_') {
            Some(p) => (p.0.to_string(), p.1.to_string()),
            None => continue,
        };
        let (sync_pending, sync_failed, sync_errors) = sync_map
            .get(key)
            .cloned()
            .unwrap_or((0, 0, Vec::new()));
        let health = if ifx_status == "P" { "unprocessed" } else { "ok" };
        rows.push(SyncStatusRow {
            part_no,
            pool_no,
            part_key: key.clone(),
            fname: fname.clone(),
            lname: lname.clone(),
            review_type: review_type.clone(),
            ifx_status: Some(ifx_status.clone()),
            pg_status: None,
            pg_decision: None,
            sync_pending,
            sync_failed,
            sync_errors,
            health: health.to_string(),
            health_reason: None,
        });
    }

    let total = rows.len();
    let error_count = rows.iter().filter(|r| r.health == "error").count();
    let warning_count = rows.iter().filter(|r| r.health == "warning").count();
    let syncing_count = rows.iter().filter(|r| r.health == "syncing").count();
    let unprocessed_count = rows.iter().filter(|r| r.health == "unprocessed").count();

    Ok(Json(SyncStatusResponse {
        rows,
        total,
        error_count,
        warning_count,
        syncing_count,
        unprocessed_count,
    }))
}

// ── Unified queue ─────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct UnifiedQueueParams {
    #[serde(rename = "type")]
    review_type: Option<String>,
    status: Option<String>,
}

/// GET /api/reviews/queue  — any authenticated user.
/// Optional: ?type=excuse|disqualify  ?status=pending_admin|pending_ceo|completed|sent_back
pub async fn unified_queue_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    axum::extract::Query(params): axum::extract::Query<UnifiedQueueParams>,
) -> ApiResult<UnifiedReviewQueue> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    #[derive(sqlx::FromRow)]
    struct ConfigRow { key: String, value: String }
    let cfg_rows: Vec<ConfigRow> = sqlx::query_as(
        "SELECT key, value FROM app_config \
         WHERE key IN ('ceo_review_state', 'show_review_notes', 'show_send_back')",
    )
    .fetch_all(pg)
    .await
    .unwrap_or_default();
    let cfg: std::collections::HashMap<String, String> =
        cfg_rows.into_iter().map(|r| (r.key, r.value)).collect();

    let maintenance  = cfg.get("ceo_review_state").map(|v| v == "maintenance").unwrap_or(false);
    let show_notes     = cfg.get("show_review_notes").map(|v| v != "false").unwrap_or(true);
    let show_send_back = cfg.get("show_send_back").map(|v| v != "false").unwrap_or(true);

    let type_filter = params.review_type.filter(|t| t != "all" && !t.is_empty());
    let status_filter = params.status.filter(|s| s != "all" && !s.is_empty());

    #[derive(sqlx::FromRow)]
    struct PgRow {
        id: Uuid,
        part_no: String,
        pool_no: String,
        part_key: String,
        review_type: String,
        status: String,
        admin_notes: Option<String>,
        ceo_notes: Option<String>,
        decision: Option<String>,
        sent_to_ceo_at: Option<chrono::DateTime<chrono::Utc>>,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let pg_rows = sqlx::query_as::<_, PgRow>(
        "SELECT id, part_no, pool_no, part_key, review_type, status, \
         admin_notes, ceo_notes, decision, sent_to_ceo_at, created_at \
         FROM status_reviews \
         WHERE ($1::text IS NULL OR review_type = $1) \
           AND ($2::text IS NULL OR status = $2) \
         ORDER BY created_at DESC",
    )
    .bind(&type_filter)
    .bind(&status_filter)
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query failed: {e}")))?;

    let part_nos: Vec<i32> = pg_rows
        .iter()
        .filter_map(|r| r.part_no.parse::<i32>().ok())
        .collect::<std::collections::HashSet<i32>>()
        .into_iter()
        .collect();

    let name_map: std::collections::HashMap<i32, (Option<String>, Option<String>)> =
        if part_nos.is_empty() {
            std::collections::HashMap::new()
        } else {
            let in_clause = part_nos
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let name_sql = format!(
                "SELECT part_no, fname, lname FROM participant WHERE part_no IN ({in_clause})"
            );
            let mut map = std::collections::HashMap::new();
            if let Ok(conn) = state.env.connect(
                &state.config.dsn,
                &state.config.user,
                &state.config.password,
                ConnectionOptions::default(),
            ) {
                if let Ok(Some(mut stmt)) = conn.execute(&name_sql, ()) {
                    if let Ok(mut buf) = TextRowSet::for_cursor(500, &mut stmt, Some(256)) {
                        if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                            while let Ok(Some(batch)) = cursor.fetch() {
                                for row in 0..batch.num_rows() {
                                    if let Some(pno) = col_str(&batch, 0, row)
                                        .and_then(|s| s.parse::<i32>().ok())
                                    {
                                        map.insert(
                                            pno,
                                            (col_str(&batch, 1, row), col_str(&batch, 2, row)),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            map
        };

    let rows: Vec<UnifiedReviewRow> = pg_rows
        .into_iter()
        .map(|r| {
            let pno: i32 = r.part_no.parse().unwrap_or(0);
            let (fname, lname) = name_map.get(&pno).cloned().unwrap_or((None, None));
            UnifiedReviewRow {
                id: r.id.to_string(),
                part_no: r.part_no,
                pool_no: r.pool_no,
                part_key: r.part_key,
                fname,
                lname,
                review_type: r.review_type,
                status: r.status,
                admin_notes: r.admin_notes,
                ceo_notes: r.ceo_notes,
                decision: r.decision,
                sent_to_ceo_at: r.sent_to_ceo_at.map(|t| t.to_rfc3339()),
                created_at: r.created_at.to_rfc3339(),
            }
        })
        .collect();

    let count = rows.len();
    Ok(Json(UnifiedReviewQueue { rows, count, maintenance, show_notes, show_send_back }))
}

// ── Recall ────────────────────────────────────────────────────

/// POST /api/reviews/:part_key/recall  — pending_ceo → pending_admin.
pub async fn recall_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    AxumPath(part_key): AxumPath<String>,
) -> ApiResult<ActionResponse> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let (part_no, pool_no) = parse_part_key(&part_key)
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "Invalid part_key"))?;

    let current: Option<String> =
        sqlx::query_scalar("SELECT status FROM status_reviews WHERE part_key = $1")
            .bind(&part_key)
            .fetch_optional(pg)
            .await
            .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    if current.as_deref() != Some("pending_ceo") {
        return Err(api_err(
            StatusCode::CONFLICT,
            format!(
                "Cannot recall — status is '{}'",
                current.as_deref().unwrap_or("not found")
            ),
        ));
    }

    let affected = sqlx::query(
        "UPDATE status_reviews \
         SET status = 'pending_admin', sent_to_ceo_at = NULL, ceo_notes = NULL, \
             updated_at = now() \
         WHERE part_key = $1 AND status = 'pending_ceo'",
    )
    .bind(&part_key)
    .execute(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?
    .rows_affected();

    if affected == 0 {
        return Err(api_err(
            StatusCode::CONFLICT,
            "Record no longer pending CEO — may have been decided concurrently",
        ));
    }

    sqlx::query(
        "INSERT INTO informix_sync_queue (operation, payload) VALUES ('reopen_review_record', $1)",
    )
    .bind(serde_json::json!({ "part_no": part_no, "pool_no": pool_no }))
    .execute(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Sync queue: {e}")))?;

    sqlx::query(
        "INSERT INTO review_history \
         (status_review_id, part_no, review_type, action, actor_sub, actor_email) \
         SELECT id, part_no, review_type, 'recalled', $1, $2 \
         FROM status_reviews WHERE part_key = $3",
    )
    .bind(&user.sub)
    .bind(&user.email)
    .bind(&part_key)
    .execute(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("History: {e}")))?;

    info!(part_key = %part_key, actor = %user.sub, "Review recalled from CEO queue");
    Ok(Json(ActionResponse { ok: true, message: "Recalled to admin queue".to_string() }))
}

// ── Sync now ──────────────────────────────────────────────────

// ── Review report ────────────────────────────────────────────

/// GET /api/reviews/report
/// All completed/sent_back reviews in the last 90 days, with aggregated sync status.
pub async fn review_report_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<ReviewReport> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    #[derive(sqlx::FromRow)]
    struct ReportRow {
        id: Uuid,
        part_no: String,
        pool_no: String,
        part_key: String,
        review_type: String,
        status: String,
        decision: Option<String>,
        decided_at: Option<chrono::DateTime<chrono::Utc>>,
        sync_status: String,
    }

    let pg_rows = sqlx::query_as::<_, ReportRow>(
        "SELECT \
            sr.id, sr.part_no, sr.pool_no, sr.part_key, sr.review_type, \
            sr.status, sr.decision, sr.decided_at, \
            CASE \
                WHEN COUNT(sq.id) = 0 THEN 'no_ops' \
                WHEN COUNT(sq.id) FILTER (WHERE sq.status = 'failed')  > 0 THEN 'failed' \
                WHEN COUNT(sq.id) FILTER (WHERE sq.status = 'pending') > 0 THEN 'pending' \
                ELSE 'done' \
            END AS sync_status \
         FROM status_reviews sr \
         LEFT JOIN informix_sync_queue sq \
             ON sq.payload->>'part_no' = sr.part_no \
            AND sq.payload->>'pool_no' = sr.pool_no \
         WHERE sr.status IN ('completed', 'sent_back') \
           AND sr.decided_at >= now() - interval '90 days' \
         GROUP BY sr.id, sr.part_no, sr.pool_no, sr.part_key, sr.review_type, \
                  sr.status, sr.decision, sr.decided_at \
         ORDER BY sr.decided_at DESC NULLS LAST",
    )
    .fetch_all(pg)
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    // Batch Informix name lookup
    let part_nos: Vec<i32> = pg_rows
        .iter()
        .filter_map(|r| r.part_no.parse::<i32>().ok())
        .collect::<std::collections::HashSet<i32>>()
        .into_iter()
        .collect();

    let name_map: std::collections::HashMap<i32, (Option<String>, Option<String>)> =
        if part_nos.is_empty() {
            std::collections::HashMap::new()
        } else {
            let in_clause = part_nos
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT part_no, fname, lname FROM participant WHERE part_no IN ({in_clause})"
            );
            let mut map = std::collections::HashMap::new();
            if let Ok(conn) = state.env.connect(
                &state.config.dsn,
                &state.config.user,
                &state.config.password,
                ConnectionOptions::default(),
            ) {
                if let Ok(Some(mut stmt)) = conn.execute(&sql, ()) {
                    if let Ok(mut buf) = TextRowSet::for_cursor(500, &mut stmt, Some(256)) {
                        if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                            while let Ok(Some(batch)) = cursor.fetch() {
                                for row in 0..batch.num_rows() {
                                    if let Some(pno) = col_str(&batch, 0, row)
                                        .and_then(|s| s.parse::<i32>().ok())
                                    {
                                        map.insert(
                                            pno,
                                            (col_str(&batch, 1, row), col_str(&batch, 2, row)),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            map
        };

    let rows: Vec<ReviewReportRow> = pg_rows
        .into_iter()
        .map(|r| {
            let pno: i32 = r.part_no.parse().unwrap_or(0);
            let (fname, lname) = name_map.get(&pno).cloned().unwrap_or((None, None));
            ReviewReportRow {
                id: r.id.to_string(),
                part_no: r.part_no,
                pool_no: r.pool_no,
                part_key: r.part_key,
                fname,
                lname,
                review_type: r.review_type,
                status: r.status,
                decision: r.decision,
                decided_at: r.decided_at.map(|t| t.to_rfc3339()),
                sync_status: r.sync_status,
            }
        })
        .collect();

    let count = rows.len();
    Ok(Json(ReviewReport { rows, count }))
}

// ── Participant check ─────────────────────────────────────────

/// GET /api/reviews/participant/:part_no
/// Cross-system snapshot: Informix pool_member + review_record,
/// PG status_reviews + sync queue + document_cache + review_history audit trail.
pub async fn participant_check_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    AxumPath(part_no): AxumPath<String>,
) -> ApiResult<ParticipantCheck> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let part_no_int: i32 = part_no
        .parse()
        .map_err(|_| api_err(StatusCode::BAD_REQUEST, "Invalid participant number"))?;

    // ── Informix ─────────────────────────────────────────────
    let (fname, lname, pool_members, review_records, documents) = {
        let mut fname: Option<String> = None;
        let mut lname: Option<String> = None;
        let mut pool_members: Vec<PoolMemberSnapshot> = Vec::new();
        let mut review_records: Vec<ReviewRecordSnapshot> = Vec::new();
        let mut documents: Vec<DocumentSnapshot> = Vec::new();

        if let Ok(conn) = state.env.connect(
            &state.config.dsn,
            &state.config.user,
            &state.config.password,
            ConnectionOptions::default(),
        ) {
            // Participant name
            let name_sql = format!(
                "SELECT fname, lname FROM participant WHERE part_no = {part_no_int}"
            );
            if let Ok(Some(mut stmt)) = conn.execute(&name_sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(2, &mut stmt, Some(100)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        if let Ok(Some(batch)) = cursor.fetch() {
                            fname = col_str(&batch, 0, 0);
                            lname = col_str(&batch, 1, 0);
                        }
                    }
                }
            }

            // Pool members (joined with pool for show_no)
            let pm_sql = format!(
                "SELECT pm.pool_no, po.show_no, pm.status, pm.scan_code \
                 FROM pool_member pm JOIN pool po ON po.pool_no = pm.pool_no \
                 WHERE pm.part_no = {part_no_int} ORDER BY pm.pool_no"
            );
            if let Ok(Some(mut stmt)) = conn.execute(&pm_sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(50, &mut stmt, Some(100)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        while let Ok(Some(batch)) = cursor.fetch() {
                            for row in 0..batch.num_rows() {
                                pool_members.push(PoolMemberSnapshot {
                                    pool_no: col_str(&batch, 0, row)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(0),
                                    show_no: col_str(&batch, 1, row)
                                        .and_then(|s| s.parse().ok()),
                                    status: col_str(&batch, 2, row)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(0),
                                    scan_code: col_str(&batch, 3, row),
                                });
                            }
                        }
                    }
                }
            }

            // Review records
            let rr_sql = format!(
                "SELECT rr_id, pool_no, review_type, status, submitted_date \
                 FROM review_record WHERE part_no = {part_no_int} \
                 ORDER BY submitted_date DESC"
            );
            if let Ok(Some(mut stmt)) = conn.execute(&rr_sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(50, &mut stmt, Some(100)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        while let Ok(Some(batch)) = cursor.fetch() {
                            for row in 0..batch.num_rows() {
                                review_records.push(ReviewRecordSnapshot {
                                    rr_id: col_str(&batch, 0, row)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(0),
                                    pool_no: col_str(&batch, 1, row)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(0),
                                    review_type: col_str(&batch, 2, row).unwrap_or_default(),
                                    ifx_status: col_str(&batch, 3, row).unwrap_or_default(),
                                    submitted_date: col_str(&batch, 4, row),
                                });
                            }
                        }
                    }
                }
            }

            // Documents from Informix part_image
            let img_sql = format!(
                "SELECT file_path, file_name FROM part_image WHERE part_no = {part_no_int}"
            );
            if let Ok(Some(mut stmt)) = conn.execute(&img_sql, ()) {
                if let Ok(mut buf) = TextRowSet::for_cursor(50, &mut stmt, Some(512)) {
                    if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                        while let Ok(Some(batch)) = cursor.fetch() {
                            for row in 0..batch.num_rows() {
                                if let (Some(fp), Some(fn_)) =
                                    (col_str(&batch, 0, row), col_str(&batch, 1, row))
                                {
                                    documents.push(DocumentSnapshot {
                                        webdav_path: format!("{fp}{fn_}"),
                                        file_name: fn_,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        (fname, lname, pool_members, review_records, documents)
    };

    // ── PostgreSQL ────────────────────────────────────────────

    #[derive(sqlx::FromRow)]
    struct SrRow {
        id: Uuid,
        pool_no: String,
        part_key: String,
        review_type: String,
        status: String,
        decision: Option<String>,
        sent_to_ceo_at: Option<chrono::DateTime<chrono::Utc>>,
        decided_at: Option<chrono::DateTime<chrono::Utc>>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let sr_rows = sqlx::query_as::<_, SrRow>(
        "SELECT id, pool_no, part_key, review_type, status, decision, \
         sent_to_ceo_at, decided_at, updated_at \
         FROM status_reviews WHERE part_no = $1 ORDER BY updated_at DESC",
    )
    .bind(&part_no)
    .fetch_all(pg)
    .await
    .unwrap_or_default();

    let status_reviews: Vec<StatusReviewSnapshot> = sr_rows
        .into_iter()
        .map(|r| StatusReviewSnapshot {
            id: r.id.to_string(),
            pool_no: r.pool_no,
            part_key: r.part_key,
            review_type: r.review_type,
            pg_status: r.status,
            decision: r.decision,
            sent_to_ceo_at: r.sent_to_ceo_at.map(|t| t.to_rfc3339()),
            decided_at: r.decided_at.map(|t| t.to_rfc3339()),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();

    #[derive(sqlx::FromRow)]
    struct SqRow {
        id: Uuid,
        operation: String,
        status: String,
        attempts: i32,
        last_error: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let sq_rows = sqlx::query_as::<_, SqRow>(
        "SELECT id, operation, status, attempts, last_error, created_at, completed_at \
         FROM informix_sync_queue \
         WHERE payload->>'part_no' = $1 \
         ORDER BY created_at DESC LIMIT 30",
    )
    .bind(&part_no)
    .fetch_all(pg)
    .await
    .unwrap_or_default();

    let sync_queue: Vec<SyncQueueSnapshot> = sq_rows
        .into_iter()
        .map(|r| SyncQueueSnapshot {
            id: r.id.to_string(),
            operation: r.operation,
            status: r.status,
            attempts: r.attempts,
            last_error: r.last_error,
            created_at: r.created_at.to_rfc3339(),
            completed_at: r.completed_at.map(|t| t.to_rfc3339()),
        })
        .collect();

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

    let hist_rows = sqlx::query_as::<_, HistRow>(
        "SELECT id, part_no, review_type, action, actor_email, notes, acted_at \
         FROM review_history WHERE part_no = $1 ORDER BY acted_at DESC",
    )
    .bind(&part_no)
    .fetch_all(pg)
    .await
    .unwrap_or_default();

    let history: Vec<ReviewHistoryEntry> = hist_rows
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

    Ok(Json(ParticipantCheck {
        part_no,
        fname,
        lname,
        pool_members,
        review_records,
        status_reviews,
        sync_queue,
        documents,
        history,
    }))
}

/// POST /api/reviews/sync-now — triggers immediate review queue refresh from Informix.
pub async fn sync_now_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
) -> ApiResult<serde_json::Value> {
    let pg = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;
    let inserted = crate::sync::refresh_review_queue(&state, pg).await;
    info!(inserted, "Manual review queue sync triggered");
    Ok(axum::Json(serde_json::json!({ "inserted": inserted })))
}
