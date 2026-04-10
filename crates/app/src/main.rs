mod routes;
mod components;

use dioxus::prelude::*;
use routes::Route;

/// Rate-limiter key extractor: tries X-Forwarded-For / X-Real-IP / peer addr
/// (via SmartIpKeyExtractor), then falls back to a shared "local" bucket
/// instead of returning an error.  This handles the case where the Vite dev
/// proxy or Dioxus's serve() hasn't populated ConnectInfo.
#[cfg(feature = "server")]
mod rate_limit {
    use std::net::IpAddr;
    use axum::http::Request;
    use tower_governor::{errors::GovernorError, key_extractor::{KeyExtractor, SmartIpKeyExtractor}};

    #[derive(Clone, Debug)]
    pub struct FallbackIpExtractor;

    impl KeyExtractor for FallbackIpExtractor {
        type Key = String;

        #[cfg(feature = "tracing")]
        fn name(&self) -> &'static str { "fallback_ip" }

        fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
            SmartIpKeyExtractor
                .extract(req)
                .map(|ip: IpAddr| ip.to_string())
                // No IP available (e.g. direct connection without ConnectInfo):
                // use a shared "local" bucket so the limiter still applies.
                .or_else(|_| Ok("local".to_string()))
        }
    }
}

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(app);

    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use std::sync::Arc;

        use anyhow::Context;
        use axum::{
            http::{HeaderName, HeaderValue},
            routing::{get, post},
        };
        use server::auth::routes::discover_oidc_client;
        use time::Duration;
        use tower_http::{
            request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
            set_header::SetResponseHeaderLayer,
        };
        use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
        use tower_sessions_sqlx_store::PostgresStore;
        use tower_governor::GovernorLayer;
        use tower_governor::governor::GovernorConfigBuilder;
        use crate::rate_limit::FallbackIpExtractor;
        use tracing::info;

        use server::auth::types::OidcConfig;
        use server::db;
        use server::AppState;

        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,audit=info".into()),
            )
            .try_init();

        let _ = dotenvy::dotenv();

        db::set_informix_env_vars();

        let config = db::AppConfig::from_env()?;
        let env = db::create_odbc_env()?;

        // Startup DB connectivity test (non-fatal)
        info!("Running startup DB connectivity test...");
        match env.connect(
            &config.dsn,
            &config.user,
            &config.password,
            server::odbc_api::ConnectionOptions::default(),
        ) {
            Ok(conn) => {
                info!("Startup DB test: connected successfully!");
                drop(conn);
            }
            Err(e) => {
                tracing::error!(error = %e, "Startup DB test failed — queries will fail until DB is reachable");
            }
        }

        // --- PostgreSQL pool (optional — degrades gracefully) ---
        let pg_pool = if let Some(ref db_url) = config.database_url {
            match sqlx::PgPool::connect(db_url).await {
                Ok(pool) => {
                    info!("PostgreSQL pool connected");
                    Some(pool)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "PostgreSQL connection failed — task/ticket features disabled");
                    None
                }
            }
        } else {
            info!("DATABASE_URL not set — task/ticket features disabled");
            None
        };

        // --- Email config (optional) ---
        let email_config = db::EmailConfig::from_env();
        if email_config.is_some() {
            info!("SMTP email configured");
        } else {
            info!("SMTP not configured — failure emails disabled");
        }

        // --- OIDC setup ---
        let oidc_config = OidcConfig::from_env()?;

        let mut http_builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none());

        if let Some(ref ca_path) = oidc_config.ca_cert_path {
            let ca_pem = std::fs::read(ca_path)
                .with_context(|| format!("Failed to read CA cert from {}", ca_path))?;
            let ca_cert = reqwest::tls::Certificate::from_pem(&ca_pem)
                .context("Failed to parse CA certificate PEM")?;
            http_builder = http_builder.add_root_certificate(ca_cert);
            info!("Custom CA certificate loaded from {}", ca_path);
        }

        let http_client = http_builder
            .build()
            .context("Failed to build HTTP client")?;

        // --- OIDC discovery (non-fatal — retried lazily on first login) ---
        info!(issuer = %oidc_config.issuer_url, "Attempting OIDC provider discovery");
        let oidc_client = match discover_oidc_client(&oidc_config, &http_client).await {
            Ok(client) => {
                info!("OIDC client configured successfully");
                Some(client)
            }
            Err(e) => {
                tracing::warn!(error = %e, "OIDC discovery failed at startup — login will retry on first request");
                None
            }
        };

        // --- WebDAV config (optional — document caching disabled when absent) ---
        let webdav_config = db::WebDavConfig::from_env();
        if webdav_config.is_some() {
            info!("WebDAV document cache configured");
        } else {
            info!("WEBDAV_BASE_URL not set — document caching disabled");
        }

        // --- Build state ---
        let state = Arc::new(AppState {
            env,
            config,
            oidc_client: tokio::sync::RwLock::new(oidc_client),
            oidc_config,
            http_client,
            pg_pool,
            email_config,
            webdav_config,
        });

        // --- Background cron tasks ---
        if let Some(ref pool) = state.pg_pool {
            // Drain informix_sync_queue every 2 minutes.
            server::sync::spawn_sync_queue_cron(state.clone(), pool.clone());
            // Pull new Informix review_record rows into PG every 5 minutes.
            server::sync::spawn_review_refresh_cron(state.clone(), pool.clone());
            info!("Background cron tasks started (sync_queue: 2 min, review_refresh: 5 min)");
        } else {
            tracing::warn!("No PostgreSQL pool — background cron tasks disabled");
        }

        // --- Rate limiting on auth routes (FISMA AC-7 / brute force prevention) ---
        // 2 req/s sustained, burst of 5 — applied per client IP.
        let governor_conf = Arc::new(
            GovernorConfigBuilder::default()
                .key_extractor(FallbackIpExtractor)
                .per_second(2)
                .burst_size(5)
                .finish()
                .expect("valid rate limiter config"),
        );
        // Periodically evict stale rate-limit entries to prevent memory growth.
        {
            let limiter = governor_conf.limiter().clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    tokio::time::Duration::from_secs(60),
                );
                loop {
                    interval.tick().await;
                    limiter.retain_recent();
                }
            });
        }

        // --- Request ID (correlation across logs and client responses) ---
        let x_request_id = HeaderName::from_static("x-request-id");

        // --- Cookie security ---
        // Set COOKIE_SECURE=false only for local HTTP dev; always true in production.
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v != "false")
            .unwrap_or(true);

        if !cookie_secure {
            tracing::warn!("COOKIE_SECURE=false — session cookies will not have Secure flag (dev only)");
        }

        // --- Public routes (rate-limited auth + health) ---
        let auth_routes: axum::Router<()> = axum::Router::new()
            .route("/auth/login", get(server::auth::routes::login_handler))
            .route("/auth/callback", get(server::auth::routes::callback_handler))
            .route("/auth/logout", post(server::auth::routes::logout_handler))
            .layer(GovernorLayer::new(governor_conf))
            .with_state(state.clone());

        let public_routes: axum::Router<()> = axum::Router::new()
            .route("/health", get(server::api::health_handler))
            .with_state(state.clone());

        // --- Protected API routes ---
        // Cache-Control: no-store on all API responses (FISMA SI-12 / data retention)
        let cache_no_store = SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        );

        let protected_routes: axum::Router<()> = axum::Router::new()
            .route("/api/current_user", get(server::api::current_user_handler))
            .route("/api/dashboard/status", get(server::api::dashboard_status_handler))
            .route("/api/dashboard/show-types", get(server::api::show_types_handler))
            .route("/api/pools/fix-show-codes", get(server::api::bad_show_codes_handler))
            .route("/api/pools/fix-show-codes", post(server::api::fix_show_code_handler))
            .route("/api/pools/blank-questionnaires", get(server::api::blank_questionnaires_handler))
            .route("/api/pools/reset-qq", post(server::api::reset_qq_handler))
            .route("/api/pools/lockouts", get(server::api::portal_lockouts_handler))
            .route("/api/pools/unlock", post(server::api::unlock_participant_handler))
            .route("/api/query_links", get(server::api::query_links_handler))
            .route("/api/queries/{slug}", get(server::api::master_list_handler))
            .route("/api/queries/{slug}/{id}", get(server::api::detail_handler))
            .route("/api/participants", get(server::api::participants_handler))
            .route("/api/pools", get(server::api::pools_handler))
            .route("/api/pools/{pool_no}/members", get(server::api::pool_members_handler))
            .route("/api/pool_staff", get(server::api::pool_staff_handler))
            .route("/api/tasks", get(server::api::tasks_handler))
            .route("/api/tasks/{id}", get(server::api::task_detail_handler))
            .route("/api/tasks/replace_staff", post(server::api::start_replace_staff_task_handler))
            .route("/api/tickets", get(server::api::user_tickets_handler))
            .route("/api/tickets/all", get(server::api::all_tickets_handler))
            // ── Phase 5: Reviews ───────────────────────────────
            // Static routes first to avoid shadowing by :part_key
            .route("/api/reviews/excuse/admin", get(server::reviews::admin_excuse_queue_handler))
            .route("/api/reviews/disqualify/admin", get(server::reviews::admin_disqualify_queue_handler))
            .route("/api/reviews/pending", get(server::reviews::pending_counts_handler))
            .route("/api/reviews/ceo", get(server::reviews::ceo_queue_handler))
            .route("/api/reviews/ceo/decide", post(server::reviews::ceo_decide_handler))
            .route("/api/reviews/ceo-state", get(server::reviews::get_ceo_state_handler))
            .route("/api/reviews/ceo-state", post(server::reviews::set_ceo_state_handler))
            .route("/api/reviews/send-to-ceo", post(server::reviews::send_to_ceo_handler))
            .route("/api/reviews/records/{part_no}", get(server::reviews::review_history_handler))
            .route("/api/reviews/sync-status", get(server::reviews::sync_status_handler))
            .route("/api/reviews/sync-status/sync/{part_key}", post(server::reviews::sync_one_handler))
            .route("/api/reviews/sync-status/lookup/{query}", get(server::reviews::lookup_handler))
            // Documents — must be before the bare /{part_key} catch-all
            .route("/api/reviews/{part_key}/documents", get(server::documents::list_documents_handler))
            .route("/api/documents/{id}", get(server::documents::serve_document_handler))
            // Parameterized last
            .route("/api/reviews/{part_key}", get(server::reviews::review_detail_handler))
            .layer(cache_no_store)
            .with_state(state.clone())
            .layer(axum::middleware::from_fn(server::auth::middleware::require_auth));

        // --- Compose the final router ---
        let base_router = dioxus::server::router(app)
            .merge(auth_routes)
            .merge(public_routes)
            .merge(protected_routes)
            .layer(axum::Extension(state.clone()))
            // Security headers applied to every response (FISMA SC-8, SC-28)
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_static("x-frame-options"),
                HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_static("referrer-policy"),
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_static("strict-transport-security"),
                HeaderValue::from_static("max-age=63072000; includeSubDomains"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                HeaderName::from_static("permissions-policy"),
                HeaderValue::from_static("geolocation=(), camera=(), microphone=()"),
            ))
            // Propagate request ID from request to response header
            .layer(PropagateRequestIdLayer::new(x_request_id.clone()))
            // Generate x-request-id UUID for every incoming request
            .layer(SetRequestIdLayer::new(x_request_id, MakeRequestUuid));

        // --- Session store ---
        // Use PostgreSQL when available (required for FISMA — sessions survive
        // restart and can be individually revoked). Falls back to in-memory
        // with a loud warning for local dev without DATABASE_URL.
        let cookie_name = if cookie_secure { "__Host-session" } else { "session" };

        let final_router = if let Some(ref pool) = state.pg_pool {
            let pg_store = PostgresStore::new(pool.clone());
            pg_store.migrate().await
                .map_err(|e| anyhow::anyhow!("Failed to create sessions table in PostgreSQL: {e}"))?;
            info!("Using PostgreSQL session store");
            base_router.layer(
                SessionManagerLayer::new(pg_store)
                    .with_name(cookie_name)
                    .with_secure(cookie_secure)
                    .with_same_site(tower_sessions::cookie::SameSite::Strict)
                    .with_http_only(true)
                    .with_expiry(Expiry::OnInactivity(Duration::hours(8))),
            )
        } else {
            tracing::warn!(
                "No PostgreSQL pool — using in-memory session store. \
                 Sessions will be lost on restart. NOT FISMA compliant."
            );
            base_router.layer(
                SessionManagerLayer::new(MemoryStore::default())
                    .with_name(cookie_name)
                    .with_secure(cookie_secure)
                    .with_same_site(tower_sessions::cookie::SameSite::Strict)
                    .with_http_only(true)
                    .with_expiry(Expiry::OnInactivity(Duration::hours(8))),
            )
        };

        Ok(final_router)
    });
}

fn app() -> Element {
    use components::toast::ToastManager;

    use_context_provider(|| Signal::new(Vec::<components::toast::Toast>::new()));

    rsx! {
        Router::<Route> {}
        ToastManager {}
    }
}
