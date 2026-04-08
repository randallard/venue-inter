use std::path::Path;
use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use serde::Deserialize;
use tracing::{error, info};

use crate::AppState;
use shared_types::{
    ActionResponse, BadShowCodeRow, BadShowCodesResponse, BlankQQResponse, BlankQQRow,
    DashboardStatus, DetailResponse, ErrorResponse, FixShowCodeParams, HealthResponse,
    MasterResponse, PoolMemberRow, PoolMembersResponse, PoolRow, PoolsResponse,
    PoolStaffResponse, PoolStaffRow, ParticipantRow, ParticipantsResponse, PortalLockoutRow,
    PortalLockoutsResponse, QueryLink, ReplaceStaffParams, ResetQQParams, ShowTypeRow,
    ShowTypesResponse, StartTaskResponse, StaffOption, TasksResponse, TicketsResponse,
    UnlockParams, UserSession,
};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

fn json_error(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

// ── GET /api/current_user ───────────────────────────────────

pub async fn current_user_handler(
    Extension(user): Extension<UserSession>,
) -> Json<UserSession> {
    Json(user)
}

// ── GET /api/query_links ────────────────────────────────────

pub async fn query_links_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<QueryLink>>, Response> {
    let config = crate::db::load_query_links(Path::new(&state.config.query_config_path))
        .map_err(|e| {
            error!(error = %e, "Failed to load query config");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to load query config")
        })?;
    Ok(Json(config.links))
}

// ── GET /api/queries/:slug ──────────────────────────────────

#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    page_size: Option<usize>,
}

pub async fn master_list_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(slug): AxumPath<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<MasterResponse>, Response> {
    let config = crate::db::load_query_links(Path::new(&state.config.query_config_path))
        .map_err(|e| {
            error!(error = %e, "Failed to load query config");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to load query config")
        })?;

    let link = config
        .links
        .iter()
        .find(|l| l.slug == slug)
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, &format!("Unknown query: {slug}")))?;

    let page = params.page.unwrap_or(0);
    let page_size = params.page_size.unwrap_or(link.master.page_size);

    let (rows, total_count) = crate::db::execute_paginated_query(
        &state.env,
        &state.config,
        &link.master.query,
        &link.master.columns,
        page,
        page_size,
    )
    .map_err(|e| {
        error!(error = %e, slug = %slug, "Query failed");
        json_error(StatusCode::INTERNAL_SERVER_ERROR, "Query failed")
    })?;

    Ok(Json(MasterResponse {
        rows,
        columns: link.master.columns.clone(),
        total_count,
        page,
        page_size,
        link_name: link.name.clone(),
    }))
}

// ── GET /api/queries/:slug/:id ──────────────────────────────

pub async fn detail_handler(
    State(state): State<Arc<AppState>>,
    AxumPath((slug, id)): AxumPath<(String, String)>,
) -> Result<Json<DetailResponse>, Response> {
    let config = crate::db::load_query_links(Path::new(&state.config.query_config_path))
        .map_err(|e| {
            error!(error = %e, "Failed to load query config");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to load query config")
        })?;

    let link = config
        .links
        .iter()
        .find(|l| l.slug == slug)
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, &format!("Unknown query: {slug}")))?;

    let detail = link
        .detail
        .as_ref()
        .ok_or_else(|| json_error(StatusCode::NOT_FOUND, &format!("No detail configured for: {slug}")))?;

    let sanitized_id: String = id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();

    if sanitized_id.is_empty() {
        return Err(json_error(StatusCode::BAD_REQUEST, "Invalid ID value"));
    }

    let sql = detail.query.replace(":id", &format!("'{sanitized_id}'"));

    let rows = crate::db::execute_query(&state.env, &state.config, &sql, &detail.columns)
        .map_err(|e| {
            error!(error = %e, slug = %slug, id = %id, "Detail query failed");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Detail query failed")
        })?;

    Ok(Json(DetailResponse {
        rows,
        columns: detail.columns.clone(),
        id_value: id,
        link_name: link.name.clone(),
    }))
}

fn api_err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

fn col_str(batch: &TextRowSet, col: usize, row: usize) -> Option<String> {
    batch
        .at(col, row)
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
}

pub async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<HealthResponse> {
    info!("Health check: connecting to DSN={}", state.config.dsn);

    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "Health check: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    let mut stmt = conn
        .execute("SELECT DBSERVERNAME FROM systables WHERE tabid = 1", ())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("Health query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(256))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut server_name = None;
    if let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))? {
        server_name = col_str(&batch, 0, 0);
    }

    info!(server = ?server_name, "Health check passed");
    Ok(Json(HealthResponse { status: "ok".into(), server_name }))
}

/// GET /api/participants — list all active participants
pub async fn participants_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<ParticipantsResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "Participants: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    let sql = "SELECT part_no, fname, lname, city, state, gender, race_code, active, date_added \
               FROM participant ORDER BY lname, fname";
    info!("Executing participants query");

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(100, &mut stmt, Some(4096))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut participants = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))? {
        for row in 0..batch.num_rows() {
            participants.push(ParticipantRow {
                part_no:    col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                fname:      col_str(&batch, 1, row),
                lname:      col_str(&batch, 2, row),
                city:       col_str(&batch, 3, row),
                state:      col_str(&batch, 4, row),
                gender:     col_str(&batch, 5, row),
                race_code:  col_str(&batch, 6, row),
                active:     col_str(&batch, 7, row),
                date_added: col_str(&batch, 8, row),
            });
        }
    }

    let count = participants.len();
    info!(count, "Participants query complete");
    Ok(Json(ParticipantsResponse { participants, count }))
}

/// GET /api/pools — active pools (ret_date >= today)
pub async fn pools_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<PoolsResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "Pools: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    let sql = "SELECT p.pool_no, p.show_no, p.ret_date, p.div_code, p.office, p.capacity, \
               COUNT(pm.pm_id) AS member_count \
               FROM pool p \
               LEFT JOIN pool_member pm ON pm.pool_no = p.pool_no \
               WHERE p.ret_date >= TODAY \
               GROUP BY p.pool_no, p.show_no, p.ret_date, p.div_code, p.office, p.capacity \
               ORDER BY p.ret_date";
    info!("Executing active pools query");

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(50, &mut stmt, Some(4096))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut pools = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))? {
        for row in 0..batch.num_rows() {
            pools.push(PoolRow {
                pool_no:      col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                show_no:      col_str(&batch, 1, row).and_then(|s| s.parse().ok()),
                ret_date:     col_str(&batch, 2, row),
                div_code:     col_str(&batch, 3, row),
                office:       col_str(&batch, 4, row),
                capacity:     col_str(&batch, 5, row).and_then(|s| s.parse().ok()),
                member_count: col_str(&batch, 6, row).and_then(|s| s.parse().ok()).unwrap_or(0),
            });
        }
    }

    let count = pools.len();
    info!(count, "Active pools query complete");
    Ok(Json(PoolsResponse { pools, count }))
}

/// GET /api/pools/:pool_no/members — participants in a pool
pub async fn pool_members_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(pool_no): axum::extract::Path<i32>,
) -> ApiResult<PoolMembersResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "PoolMembers: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    let sql = format!(
        "SELECT pm.pm_id, pm.pool_no, pm.part_no, p.fname, p.lname, pm.status, pm.rand_nbr, pm.responded \
         FROM pool_member pm \
         JOIN participant p ON p.part_no = pm.part_no \
         WHERE pm.pool_no = {pool_no} \
         ORDER BY p.lname, p.fname"
    );
    info!(pool_no, "Executing pool members query");

    let mut stmt = conn.execute(&sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(4096))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut members = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))? {
        for row in 0..batch.num_rows() {
            members.push(PoolMemberRow {
                pm_id:    col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                pool_no:  col_str(&batch, 1, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                part_no:  col_str(&batch, 2, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                fname:    col_str(&batch, 3, row),
                lname:    col_str(&batch, 4, row),
                status:   col_str(&batch, 5, row).and_then(|s| s.parse().ok()).unwrap_or(1),
                rand_nbr: col_str(&batch, 6, row).and_then(|s| s.parse().ok()),
                responded: col_str(&batch, 7, row),
            });
        }
    }

    let count = members.len();
    info!(count, pool_no, "Pool members query complete");
    Ok(Json(PoolMembersResponse { members, count, pool_no }))
}

/// GET /api/pool_staff — staff/contacts assigned to future pool sessions
pub async fn pool_staff_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<PoolStaffResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "PoolStaff: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    // Query session_resources for future-dated staff assignments.
    // session_resources mirrors the pool session staffing pattern.
    let sql = "SELECT sr_name, sr_type, COUNT(*) AS schedule_count, \
               MIN(sr_datetime_start) AS first_date, MAX(sr_datetime_start) AS last_date \
               FROM session_resources \
               WHERE sr_datetime_start >= CURRENT YEAR TO MINUTE \
               GROUP BY sr_name, sr_type \
               ORDER BY sr_name, sr_type";
    info!("Executing pool staff query");

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(512))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch: {e}")))? {
        for row in 0..batch.num_rows() {
            rows.push(PoolStaffRow {
                ct_name:        col_str(&batch, 0, row).unwrap_or_default(),
                ct_type:        col_str(&batch, 1, row).unwrap_or_default(),
                schedule_count: col_str(&batch, 2, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                first_date:     col_str(&batch, 3, row),
                last_date:      col_str(&batch, 4, row),
                has_codes_entry: false,
                codes_options:  Vec::new(),
            });
        }
    }
    drop(cursor);
    drop(buf);

    let mut types_seen = std::collections::HashSet::new();
    let mut codes_by_type: std::collections::HashMap<String, Vec<StaffOption>> = std::collections::HashMap::new();

    for r in &rows {
        if types_seen.insert(r.ct_type.clone()) {
            let co_type = {
                let mut c = r.ct_type.chars();
                match c.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().to_string() + c.as_str(),
                }
            };
            let codes_sql = format!(
                "SELECT sc_code, sc_translation FROM staff_codes WHERE sc_type = '{}' ORDER BY sc_translation",
                co_type.replace('\'', "''")
            );
            if let Ok(Some(mut cstmt)) = conn.execute(&codes_sql, ()) {
                let mut cbuf = TextRowSet::for_cursor(200, &mut cstmt, Some(4096)).ok();
                if let Some(ref mut cb) = cbuf {
                    if let Ok(mut ccursor) = cstmt.bind_buffer(cb) {
                        let mut options = Vec::new();
                        while let Ok(Some(cbatch)) = ccursor.fetch() {
                            for crow in 0..cbatch.num_rows() {
                                options.push(StaffOption {
                                    co_code:        col_str(&cbatch, 0, crow).unwrap_or_default(),
                                    co_translation: col_str(&cbatch, 1, crow).unwrap_or_default(),
                                });
                            }
                        }
                        codes_by_type.insert(r.ct_type.clone(), options);
                    }
                }
            }
        }
    }

    for r in &mut rows {
        let options = codes_by_type.get(&r.ct_type).cloned().unwrap_or_default();
        r.has_codes_entry = options.iter().any(|o| o.co_translation == r.ct_name);
        r.codes_options = options;
    }

    let count = rows.len();
    info!(count, "Pool staff query complete");
    Ok(Json(PoolStaffResponse { rows, count }))
}

/// GET /api/tasks — tasks for the authenticated user
pub async fn tasks_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
) -> ApiResult<TasksResponse> {
    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let tasks = crate::tasks::get_tasks_for_user(pool, &user.sub)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    let count = tasks.len();
    Ok(Json(TasksResponse { tasks, count }))
}

/// GET /api/tasks/:id — single task
pub async fn task_detail_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> ApiResult<shared_types::TaskRow> {
    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|_| api_err(StatusCode::BAD_REQUEST, "Invalid task ID"))?;

    let task = crate::tasks::get_task_by_id(pool, uuid)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "Task not found"))?;

    Ok(Json(task))
}

/// POST /api/tasks/replace_staff — replace a staff member across future pool sessions
pub async fn start_replace_staff_task_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<ReplaceStaffParams>,
) -> ApiResult<StartTaskResponse> {
    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let description = format!(
        "Replace {} '{}' with '{}'",
        params.ct_type, params.old_name, params.new_name
    );

    let task_id = crate::tasks::create_task(
        pool,
        &user.sub,
        user.email.as_deref(),
        &description,
        "replace_staff",
        &serde_json::to_value(&params).unwrap_or_default(),
    )
    .await
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create task: {e}")))?;

    crate::audit::task_created(&user.sub, &task_id.to_string(), "replace_staff", &description, None);

    crate::task_runner::spawn_replace_staff_task(
        state.clone(),
        pool.clone(),
        task_id,
        params,
        user.sub.clone(),
        user.email.clone(),
    );

    Ok(Json(StartTaskResponse {
        task_id: task_id.to_string(),
        message: "Task started".to_string(),
    }))
}

/// GET /api/tickets — tickets for the authenticated user
pub async fn user_tickets_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
) -> ApiResult<TicketsResponse> {
    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let tickets = crate::tickets::get_tickets_for_user(pool, &user.sub)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    let count = tickets.len();
    Ok(Json(TicketsResponse { tickets, count }))
}

/// GET /api/tickets/all — all tickets (helpdesk group only)
pub async fn all_tickets_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
) -> ApiResult<TicketsResponse> {
    if !user.groups.iter().any(|g| g == "helpdesk") {
        return Err(api_err(StatusCode::FORBIDDEN, "Helpdesk group required"));
    }

    crate::audit::admin_access(&user.sub, "/api/tickets/all", None);

    let pool = state.pg_pool.as_ref()
        .ok_or_else(|| api_err(StatusCode::SERVICE_UNAVAILABLE, "PostgreSQL not configured"))?;

    let tickets = crate::tickets::get_all_tickets(pool)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?;

    let count = tickets.len();
    Ok(Json(TicketsResponse { tickets, count }))
}

// ── Phase 2: Dashboard & Operational ───────────────────────

/// Shared helper: run a single COUNT(*) query on an open Informix connection.
fn ifx_count(
    conn: &odbc_api::Connection<'_>,
    sql: &str,
) -> Result<i64, (StatusCode, Json<ErrorResponse>)> {
    let mut stmt = conn
        .execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Count query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set for count"))?;

    let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(128))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let count = if let Some(batch) = cursor.fetch()
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch error: {e}")))?
    {
        batch.at(0, 0)
            .map(|b| String::from_utf8_lossy(b).trim().parse::<i64>().unwrap_or(0))
            .unwrap_or(0)
    } else {
        0
    };
    Ok(count)
}

/// GET /api/dashboard/status
pub async fn dashboard_status_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<DashboardStatus> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| { error!(error = %e, "Dashboard: connect failed"); api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")) })?;

    let bad_show_codes = ifx_count(&conn,
        "SELECT COUNT(*) FROM pool_member pm \
         JOIN pool po ON po.pool_no = pm.pool_no \
         WHERE po.div_code NOT IN (SELECT st_code FROM show_type) \
         AND po.ret_date >= TODAY")?;

    let blank_questionnaires = ifx_count(&conn,
        "SELECT COUNT(*) FROM pool_member pm \
         JOIN pool po ON po.pool_no = pm.pool_no \
         WHERE pm.responded = 'N' AND pm.status = 1 AND po.ret_date >= TODAY")?;

    let portal_lockouts = ifx_count(&conn,
        "SELECT COUNT(DISTINCT p.part_no) FROM participant p \
         JOIN pool_member pm ON pm.part_no = p.part_no \
         JOIN pool po ON po.pool_no = pm.pool_no \
         WHERE p.active = 'I' AND po.ret_date >= TODAY")?;

    drop(conn);

    let (informix_sync_pending, informix_sync_failed) = if let Some(pg) = &state.pg_pool {
        let row: (Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT \
               COUNT(*) FILTER (WHERE status = 'pending'), \
               COUNT(*) FILTER (WHERE status = 'failed') \
             FROM informix_sync_queue WHERE status != 'completed'"
        )
        .fetch_one(pg)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("PG query failed: {e}")))?;
        (row.0.unwrap_or(0), row.1.unwrap_or(0))
    } else {
        (0, 0)
    };

    info!(bad_show_codes, blank_questionnaires, portal_lockouts, informix_sync_pending, informix_sync_failed, "Dashboard status");
    Ok(Json(DashboardStatus { bad_show_codes, blank_questionnaires, portal_lockouts, informix_sync_pending, informix_sync_failed }))
}

/// GET /api/dashboard/show-types — valid codes for the fix-show-code form
pub async fn show_types_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<ShowTypesResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    let mut stmt = conn.execute(
        "SELECT st_code, st_description FROM show_type ORDER BY st_description", ()
    )
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
    .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(20, &mut stmt, Some(256))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch: {e}")))? {
        for row in 0..batch.num_rows() {
            rows.push(ShowTypeRow {
                st_code:        col_str(&batch, 0, row).unwrap_or_default(),
                st_description: col_str(&batch, 1, row).unwrap_or_default(),
            });
        }
    }
    Ok(Json(ShowTypesResponse { rows }))
}

/// GET /api/pools/fix-show-codes
pub async fn bad_show_codes_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<BadShowCodesResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    let sql = "SELECT pm.pm_id, pm.pool_no, pm.part_no, p.fname, p.lname, po.div_code \
               FROM pool_member pm \
               JOIN participant p ON p.part_no = pm.part_no \
               JOIN pool po ON po.pool_no = pm.pool_no \
               WHERE po.div_code NOT IN (SELECT st_code FROM show_type) \
               AND po.ret_date >= TODAY \
               ORDER BY p.lname, p.fname";

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(512))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch: {e}")))? {
        for row in 0..batch.num_rows() {
            rows.push(BadShowCodeRow {
                pm_id:    col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                pool_no:  col_str(&batch, 1, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                part_no:  col_str(&batch, 2, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                fname:    col_str(&batch, 3, row),
                lname:    col_str(&batch, 4, row),
                bad_code: col_str(&batch, 5, row),
            });
        }
    }
    let count = rows.len();
    Ok(Json(BadShowCodesResponse { rows, count }))
}

/// POST /api/pools/fix-show-codes
pub async fn fix_show_code_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<FixShowCodeParams>,
) -> ApiResult<ActionResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    // Validate new_code is a real show type before executing
    let valid = ifx_count(&conn, &format!(
        "SELECT COUNT(*) FROM show_type WHERE st_code = '{}'",
        params.new_code.replace('\'', "''")
    ))?;
    if valid == 0 {
        return Err(api_err(StatusCode::BAD_REQUEST, &format!("'{}' is not a valid show type code", params.new_code)));
    }

    conn.execute(&format!(
        "UPDATE pool SET div_code = '{}' WHERE pool_no = {}",
        params.new_code.replace('\'', "''"), params.pool_no
    ), ())
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?;

    info!(pool_no = params.pool_no, new_code = %params.new_code, actor = %user.sub, "Fixed show code");
    Ok(Json(ActionResponse { ok: true, message: format!("Pool {} updated to {}", params.pool_no, params.new_code) }))
}

/// GET /api/pools/blank-questionnaires
pub async fn blank_questionnaires_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<BlankQQResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    let sql = "SELECT pm.pm_id, pm.pool_no, pm.part_no, p.fname, p.lname, po.ret_date \
               FROM pool_member pm \
               JOIN participant p ON p.part_no = pm.part_no \
               JOIN pool po ON po.pool_no = pm.pool_no \
               WHERE pm.responded = 'N' AND pm.status = 1 AND po.ret_date >= TODAY \
               ORDER BY po.ret_date, p.lname, p.fname";

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(500, &mut stmt, Some(512))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch: {e}")))? {
        for row in 0..batch.num_rows() {
            rows.push(BlankQQRow {
                pm_id:    col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                pool_no:  col_str(&batch, 1, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                part_no:  col_str(&batch, 2, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                fname:    col_str(&batch, 3, row),
                lname:    col_str(&batch, 4, row),
                ret_date: col_str(&batch, 5, row),
            });
        }
    }
    let count = rows.len();
    Ok(Json(BlankQQResponse { rows, count }))
}

/// POST /api/pools/reset-qq
pub async fn reset_qq_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<ResetQQParams>,
) -> ApiResult<ActionResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    conn.execute(&format!(
        "UPDATE pool_member SET responded = 'N', scan_code = NULL WHERE pm_id = {}",
        params.pm_id
    ), ())
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?;

    info!(pm_id = params.pm_id, actor = %user.sub, "Reset questionnaire");
    Ok(Json(ActionResponse { ok: true, message: format!("Questionnaire reset for member {}", params.pm_id) }))
}

/// GET /api/pools/lockouts
pub async fn portal_lockouts_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<PortalLockoutsResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    let sql = "SELECT DISTINCT p.part_no, p.fname, p.lname \
               FROM participant p \
               JOIN pool_member pm ON pm.part_no = p.part_no \
               JOIN pool po ON po.pool_no = pm.pool_no \
               WHERE p.active = 'I' AND po.ret_date >= TODAY \
               ORDER BY p.lname, p.fname";

    let mut stmt = conn.execute(sql, ())
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Query failed: {e}")))?
        .ok_or_else(|| api_err(StatusCode::INTERNAL_SERVER_ERROR, "No result set"))?;

    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(256))
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Buffer error: {e}")))?;
    let mut cursor = stmt.bind_buffer(&mut buf)
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Bind error: {e}")))?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Fetch: {e}")))? {
        for row in 0..batch.num_rows() {
            rows.push(PortalLockoutRow {
                part_no: col_str(&batch, 0, row).and_then(|s| s.parse().ok()).unwrap_or(0),
                fname:   col_str(&batch, 1, row),
                lname:   col_str(&batch, 2, row),
            });
        }
    }
    let count = rows.len();
    Ok(Json(PortalLockoutsResponse { rows, count }))
}

/// POST /api/pools/unlock
pub async fn unlock_participant_handler(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<UserSession>,
    axum::extract::Json(params): axum::extract::Json<UnlockParams>,
) -> ApiResult<ActionResponse> {
    let conn = state.env
        .connect(&state.config.dsn, &state.config.user, &state.config.password, ConnectionOptions::default())
        .map_err(|e| api_err(StatusCode::SERVICE_UNAVAILABLE, format!("DB connection failed: {e}")))?;

    conn.execute(&format!(
        "UPDATE participant SET active = 'A' WHERE part_no = {}", params.part_no
    ), ())
    .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("Update failed: {e}")))?;

    info!(part_no = params.part_no, actor = %user.sub, "Unlocked portal account");
    Ok(Json(ActionResponse { ok: true, message: format!("Participant {} portal access restored", params.part_no) }))
}
