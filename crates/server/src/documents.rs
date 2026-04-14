use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    Extension,
};
use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use shared_types::{ErrorResponse, UserSession};

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
    pub file_name: String,
    pub webdav_path: String,
}

#[derive(Serialize)]
pub struct DocumentsResponse {
    /// From pool_member.scan_code: None = not received, "web" = portal, other = scan batch code.
    pub scan_code: Option<String>,
    pub documents: Vec<DocumentMeta>,
}

/// GET /api/reviews/:part_key/documents
///
/// Lists documents for a participant from Informix part_image. For each file,
/// upserts a row into document_cache and fires a background WebDAV fetch if
/// not yet cached — so the proxy endpoint can serve from cache during the
/// active review window.
///
/// Cache cleanup (delete document_cache rows for the participant) is handled
/// by the Phase 7 sync cron after Informix sync ops complete successfully.
pub async fn list_documents_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    axum::extract::Path(part_key): axum::extract::Path<String>,
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

    let mut scan_code: Option<String> = None;
    let mut ifx_files: Vec<(String, String)> = Vec::new(); // (file_path, file_name)

    if let Ok(conn) = state.env.connect(
        &state.config.dsn,
        &state.config.user,
        &state.config.password,
        ConnectionOptions::default(),
    ) {
        let sc_sql = format!(
            "SELECT scan_code FROM pool_member WHERE part_no = {part_no} AND pool_no = {pool_no}"
        );
        if let Ok(Some(mut stmt)) = conn.execute(&sc_sql, ()) {
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
        return Ok(Json(DocumentsResponse { scan_code, documents: Vec::new() }));
    }

    // Upsert cache rows and kick off background fetch for any not yet cached.
    let mut documents: Vec<DocumentMeta> = Vec::new();

    for (file_path, file_name) in &ifx_files {
        let webdav_path = format!("{file_path}{file_name}");

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
        }

        let row = sqlx::query_as::<_, CacheRow>(
            "SELECT id, fetch_status FROM document_cache WHERE webdav_path = $1",
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

        documents.push(DocumentMeta { file_name: file_name.clone(), webdav_path });
    }

    Ok(Json(DocumentsResponse { scan_code, documents }))
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

/// Decode TIFF bytes and re-encode as JPEG for browser rendering.
fn convert_to_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    use image::ImageFormat;
    use std::io::Cursor;
    let img = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg).ok()?;
    Some(out)
}

#[derive(Deserialize)]
pub struct ProxyParams {
    path: String,
}

/// GET /api/documents/view?path=<webdav_path>
///
/// Serves a document for viewing. Checks document_cache first (populated
/// during active review); if not cached, proxies directly from WebDAV.
/// Nothing is written to the cache here — post-review views are always
/// fresh from the document storage server.
pub async fn proxy_document_handler(
    State(state): State<Arc<AppState>>,
    Extension(_user): Extension<UserSession>,
    Query(params): Query<ProxyParams>,
) -> Response {
    if params.path.contains("..") || params.path.starts_with('/') {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Check cache first
    if let Some(pg) = &state.pg_pool {
        #[derive(sqlx::FromRow)]
        struct CacheRow {
            data: Option<Vec<u8>>,
            file_name: String,
            fetch_status: String,
        }

        if let Ok(Some(row)) = sqlx::query_as::<_, CacheRow>(
            "SELECT data, file_name, fetch_status \
             FROM document_cache WHERE webdav_path = $1",
        )
        .bind(&params.path)
        .fetch_optional(pg)
        .await
        {
            if row.fetch_status == "cached" {
                if let Some(data) = row.data {
                    let (content_type, body) = match convert_to_jpeg(&data) {
                        Some(jpeg) => ("image/jpeg", jpeg),
                        None => ("image/tiff", data),
                    };
                    let stem = row.file_name.trim_end_matches(".tif").trim_end_matches(".tiff");
                    let disposition = format!("inline; filename=\"{stem}.jpg\"");
                    return Response::builder()
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::CONTENT_DISPOSITION, disposition)
                        .body(axum::body::Body::from(body))
                        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
                }
            }
        }
    }

    // Cache miss or no cache — proxy directly from WebDAV
    let wdav = match &state.webdav_config {
        Some(w) => w.clone(),
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "Document storage not configured")
                .into_response()
        }
    };

    let url = format!("{}/{}", wdav.base_url.trim_end_matches('/'), params.path);

    let resp = match state
        .http_client
        .get(&url)
        .basic_auth(&wdav.user, Some(&wdav.password))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, path = %params.path, "WebDAV request failed");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if !resp.status().is_success() {
        let status = if resp.status() == reqwest::StatusCode::NOT_FOUND {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::BAD_GATEWAY
        };
        return status.into_response();
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to read WebDAV response body");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let (content_type, body) = match convert_to_jpeg(&bytes) {
        Some(jpeg) => ("image/jpeg", jpeg),
        None => ("image/tiff", bytes.to_vec()),
    };

    let stem = params
        .path
        .split('/')
        .next_back()
        .unwrap_or("document")
        .trim_end_matches(".tif")
        .trim_end_matches(".tiff");
    let disposition = format!("inline; filename=\"{stem}.jpg\"");

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
