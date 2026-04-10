use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Utc};
use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use serde::Serialize;
use uuid::Uuid;

use crate::AppState;
use shared_types::ErrorResponse;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

fn api_err(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

fn col_str(batch: &TextRowSet, col: usize, row: usize) -> Option<String> {
    batch
        .at(col, row)
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
}

#[derive(Serialize)]
pub struct DocumentMeta {
    pub id: String,
    pub file_name: String,
    pub fetch_status: String,
    pub fetched_at: Option<String>,
}

#[derive(Serialize)]
pub struct DocumentsResponse {
    /// From pool.scan_code: None/"" = no submission, "web" = portal, numeric = scanned.
    pub scan_code: Option<String>,
    pub documents: Vec<DocumentMeta>,
}

/// GET /api/reviews/:part_key/documents
///
/// Lists documents for a participant's review. For each file found in
/// part_image (Informix), upserts a row into document_cache and fires a
/// background WebDAV fetch if the file hasn't been pulled yet. Returns
/// immediately with whatever cache state exists so the CEO page can render
/// without blocking on the fetch.
pub async fn list_documents_handler(
    State(state): State<Arc<AppState>>,
    Path(part_key): Path<String>,
) -> ApiResult<DocumentsResponse> {
    let pg = state.pg_pool.as_ref().ok_or_else(|| {
        api_err(StatusCode::SERVICE_UNAVAILABLE, "Database unavailable")
    })?;

    let (part_no, pool_no) = part_key
        .split_once('_')
        .and_then(|(a, b)| {
            let pn: i32 = a.parse().ok()?;
            let pl: i32 = b.parse().ok()?;
            Some((pn, pl))
        })
        .ok_or_else(|| api_err(StatusCode::BAD_REQUEST, "Invalid part_key"))?;

    // --- Query Informix for scan_code and part_image records ---
    let mut scan_code: Option<String> = None;
    let mut ifx_files: Vec<(String, String)> = Vec::new(); // (file_path, file_name)

    if let Ok(conn) = state.env.connect(
        &state.config.dsn,
        &state.config.user,
        &state.config.password,
        ConnectionOptions::default(),
    ) {
        // scan_code tells the frontend whether a questionnaire was submitted
        // online ("web"), scanned (numeric batch code), or not yet received ("").
        let pool_sql = format!("SELECT scan_code FROM pool WHERE pool_no = {pool_no}");
        if let Ok(Some(mut stmt)) = conn.execute(&pool_sql, ()) {
            if let Ok(mut buf) = TextRowSet::for_cursor(1, &mut stmt, Some(32)) {
                if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                    if let Ok(Some(batch)) = cursor.fetch() {
                        scan_code = col_str(&batch, 0, 0).filter(|s| !s.is_empty());
                    }
                }
            }
        }

        let img_sql = format!(
            "SELECT file_path, file_name FROM part_image WHERE part_no = {part_no}"
        );
        if let Ok(Some(mut stmt)) = conn.execute(&img_sql, ()) {
            if let Ok(mut buf) = TextRowSet::for_cursor(50, &mut stmt, Some(512)) {
                if let Ok(mut cursor) = stmt.bind_buffer(&mut buf) {
                    while let Ok(Some(batch)) = cursor.fetch() {
                        for row in 0..batch.num_rows() {
                            if let (Some(fp), Some(fn_)) =
                                (col_str(&batch, 0, row), col_str(&batch, 1, row))
                            {
                                ifx_files.push((fp, fn_));
                            }
                        }
                    }
                }
            }
        }
    }

    if ifx_files.is_empty() {
        return Ok(Json(DocumentsResponse {
            scan_code,
            documents: Vec::new(),
        }));
    }

    // --- Upsert cache rows; kick off background fetch for any that are pending ---
    let mut docs: Vec<DocumentMeta> = Vec::new();

    for (file_path, file_name) in &ifx_files {
        // file_path from Informix includes trailing slash (e.g. "21A/2023/batch/")
        let webdav_path = format!("{}{}", file_path, file_name);

        sqlx::query(
            "INSERT INTO document_cache (part_no, webdav_path, file_name) \
             VALUES ($1, $2, $3) ON CONFLICT (webdav_path) DO NOTHING",
        )
        .bind(part_no.to_string())
        .bind(&webdav_path)
        .bind(file_name)
        .execute(pg)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

        #[derive(sqlx::FromRow)]
        struct CacheRow {
            id: Uuid,
            fetch_status: String,
            fetched_at: Option<DateTime<Utc>>,
        }

        let row = sqlx::query_as::<_, CacheRow>(
            "SELECT id, fetch_status, fetched_at FROM document_cache WHERE webdav_path = $1",
        )
        .bind(&webdav_path)
        .fetch_one(pg)
        .await
        .map_err(|e| api_err(StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

        if row.fetch_status == "pending" {
            if let Some(wdav) = &state.webdav_config {
                tokio::spawn(fetch_and_cache(
                    state.http_client.clone(),
                    pg.clone(),
                    wdav.base_url.clone(),
                    wdav.user.clone(),
                    wdav.password.clone(),
                    webdav_path.clone(),
                    row.id,
                ));
            } else {
                tracing::warn!(
                    webdav_path,
                    "WEBDAV_BASE_URL not configured — document fetch skipped"
                );
            }
        }

        docs.push(DocumentMeta {
            id: row.id.to_string(),
            file_name: file_name.clone(),
            fetch_status: row.fetch_status,
            fetched_at: row.fetched_at.map(|t| t.to_rfc3339()),
        });
    }

    Ok(Json(DocumentsResponse { scan_code, documents: docs }))
}

async fn fetch_and_cache(
    http_client: reqwest::Client,
    pg_pool: sqlx::PgPool,
    base_url: String,
    user: String,
    password: String,
    webdav_path: String,
    id: Uuid,
) {
    let url = format!("{}/{}", base_url.trim_end_matches('/'), webdav_path);

    match http_client
        .get(&url)
        .basic_auth(&user, Some(&password))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.bytes().await {
            Ok(bytes) => {
                let _ = sqlx::query(
                    "UPDATE document_cache \
                     SET data = $1, fetch_status = 'cached', fetched_at = now() \
                     WHERE id = $2",
                )
                .bind(bytes.as_ref())
                .bind(id)
                .execute(&pg_pool)
                .await;
                tracing::info!(doc_id = %id, bytes = bytes.len(), "Document cached from WebDAV");
            }
            Err(e) => {
                set_failed(&pg_pool, id, &e.to_string()).await;
                tracing::error!(doc_id = %id, error = %e, "Failed to read WebDAV response body");
            }
        },
        Ok(resp) => {
            set_failed(&pg_pool, id, &format!("HTTP {}", resp.status())).await;
            tracing::error!(doc_id = %id, status = %resp.status(), "WebDAV returned non-success");
        }
        Err(e) => {
            set_failed(&pg_pool, id, &e.to_string()).await;
            tracing::error!(doc_id = %id, error = %e, "WebDAV request failed");
        }
    }
}

async fn set_failed(pool: &sqlx::PgPool, id: Uuid, error: &str) {
    let _ = sqlx::query(
        "UPDATE document_cache SET fetch_status = 'failed', fetch_error = $1 WHERE id = $2",
    )
    .bind(error)
    .bind(id)
    .execute(pool)
    .await;
}

/// GET /api/documents/:id
///
/// Serves a cached document by UUID. Returns 202 Accepted while the
/// background fetch is still in progress so the frontend can poll.
pub async fn serve_document_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Response {
    let pg = match state.pg_pool.as_ref() {
        Some(p) => p,
        None => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    #[derive(sqlx::FromRow)]
    struct DocRow {
        file_name: String,
        fetch_status: String,
        data: Option<Vec<u8>>,
    }

    let row = match sqlx::query_as::<_, DocRow>(
        "SELECT file_name, fetch_status, data FROM document_cache WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pg)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match row.fetch_status.as_str() {
        "cached" => {
            let data = match row.data {
                Some(d) => d,
                None => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
            let disposition = format!("inline; filename=\"{}\"", row.file_name);
            Response::builder()
                .header(header::CONTENT_TYPE, "image/tiff")
                .header(header::CONTENT_DISPOSITION, disposition)
                .body(axum::body::Body::from(data))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        "pending" => StatusCode::ACCEPTED.into_response(),
        "failed" => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
