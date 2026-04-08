use anyhow::{Context, Result};
use odbc_api::{buffers::TextRowSet, ConnectionOptions, Cursor, Environment};
use shared_types::{ColumnDef, QueryLinksConfig};
use std::env;
use std::path::Path;
use tracing::info;

pub struct AppConfig {
    pub dsn: String,
    pub user: String,
    pub password: String,
    pub port: u16,
    pub instance: String,
    pub query_config_path: String,
    pub database_url: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let instance = env::var("APP_INSTANCE").unwrap_or_else(|_| "test".into());
        Ok(Self {
            dsn: env::var("INFORMIX_DSN").unwrap_or_else(|_| "venueinter_dev".into()),
            user: env::var("INFORMIX_USER").context("INFORMIX_USER must be set")?,
            password: env::var("INFORMIX_PASSWORD").context("INFORMIX_PASSWORD must be set")?,
            port: env::var("APP_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .context("APP_PORT must be a valid port number")?,
            instance,
            query_config_path: env::var("QUERY_CONFIG_PATH")
                .unwrap_or_else(|_| "queries.yaml".into()),
            database_url: env::var("DATABASE_URL").ok(),
        })
    }

    pub fn connection_string(&self) -> String {
        let server = env::var("INFORMIXSERVER").unwrap_or_else(|_| "informix".into());
        format!(
            "Driver=Informix;Server={server};Database=venueinter;\
             Service=9088;Protocol=onsoctcp;\
             UID={};PWD={};CLIENT_LOCALE=en_US.UTF8;DB_LOCALE=en_US.819",
            self.user, self.password
        )
    }
}

pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
    /// Whether to use TLS (STARTTLS). Set SMTP_TLS=false for plain-text relay.
    pub smtp_tls: bool,
    pub from_address: String,
    pub sysadmin_email: String,
}

impl EmailConfig {
    pub fn from_env() -> Option<Self> {
        let host = env::var("SMTP_HOST").ok()?;
        if host.is_empty() {
            return None;
        }
        Some(Self {
            smtp_host: host,
            smtp_port: env::var("SMTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587),
            smtp_user: env::var("SMTP_USER").ok().filter(|s| !s.is_empty()),
            smtp_password: env::var("SMTP_PASSWORD").ok().filter(|s| !s.is_empty()),
            smtp_tls: env::var("SMTP_TLS").map(|v| v != "false").unwrap_or(true),
            from_address: env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@localhost".into()),
            sysadmin_email: env::var("SYSADMIN_EMAIL")
                .unwrap_or_else(|_| "admin@localhost".into()),
        })
    }
}

pub fn create_odbc_env() -> Result<Environment> {
    Environment::new().context("Failed to create ODBC environment")
}

/// Set Informix environment variables needed by the CSDK driver.
/// Preserves any values already set in the environment (e.g. from Dockerfile).
pub fn set_informix_env_vars() {
    let defaults = [
        ("INFORMIXDIR", "/opt/informix"),
        ("INFORMIXSERVER", "informix"),
        ("INFORMIXSQLHOSTS", "/opt/informix/etc/sqlhosts"),
        ("DB_LOCALE", "en_US.819"),
        ("CLIENT_LOCALE", "en_US.UTF8"),
        ("ODBCSYSINI", "/etc"),
        ("ODBCINI", "/etc/odbc.ini"),
    ];
    for (key, default) in defaults {
        if env::var(key).is_err() {
            unsafe { env::set_var(key, default) };
        }
    }
}

/// Load the query links configuration from YAML.
pub fn load_query_links(path: &Path) -> Result<QueryLinksConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read query config from {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse query config from {}", path.display()))
}

fn col_str_q(batch: &TextRowSet, col: usize, row: usize) -> String {
    batch
        .at(col, row)
        .map(|b| String::from_utf8_lossy(b).trim().to_string())
        .unwrap_or_default()
}

/// Execute a SQL query and return all rows as Vec<Vec<String>>.
pub fn execute_query(
    env: &Environment,
    config: &AppConfig,
    sql: &str,
    columns: &[ColumnDef],
) -> Result<Vec<Vec<String>>> {
    let conn = env
        .connect(&config.dsn, &config.user, &config.password, ConnectionOptions::default())
        .context("DB connection failed")?;

    info!(sql = sql, "Executing query");

    let mut stmt = conn
        .execute(sql, ())
        .context("Query execution failed")?
        .ok_or_else(|| anyhow::anyhow!("No result set"))?;

    let num_cols = columns.len();
    let mut buf = TextRowSet::for_cursor(200, &mut stmt, Some(4096))
        .context("Buffer allocation failed")?;
    let mut cursor = stmt
        .bind_buffer(&mut buf)
        .context("Buffer bind failed")?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().context("Fetch failed")? {
        for row_idx in 0..batch.num_rows() {
            let mut row = Vec::with_capacity(num_cols);
            for col_idx in 0..num_cols {
                row.push(col_str_q(&batch, col_idx, row_idx));
            }
            rows.push(row);
        }
    }

    Ok(rows)
}

/// Execute a paginated query using Informix SKIP/FIRST syntax.
/// Returns (rows, total_count).
pub fn execute_paginated_query(
    env: &Environment,
    config: &AppConfig,
    sql: &str,
    columns: &[ColumnDef],
    page: usize,
    page_size: usize,
) -> Result<(Vec<Vec<String>>, usize)> {
    let conn = env
        .connect(&config.dsn, &config.user, &config.password, ConnectionOptions::default())
        .context("DB connection failed")?;

    let count_sql = build_count_query(sql);
    info!(sql = %count_sql, "Executing count query");

    let total_count = {
        let mut stmt = conn
            .execute(&count_sql, ())
            .context("Count query failed")?
            .ok_or_else(|| anyhow::anyhow!("No result set for count"))?;

        let mut buf = TextRowSet::for_cursor(1, &mut stmt, Some(256))
            .context("Count buffer failed")?;
        let mut cursor = stmt.bind_buffer(&mut buf).context("Count bind failed")?;

        let count = if let Some(batch) = cursor.fetch().context("Count fetch failed")? {
            col_str_q(&batch, 0, 0).parse::<usize>().unwrap_or(0)
        } else {
            0
        };
        drop(cursor);
        count
    };

    let offset = page * page_size;
    let paginated_sql = build_paginated_query(sql, offset, page_size);
    info!(sql = %paginated_sql, page = page, "Executing paginated query");

    let num_cols = columns.len();
    let mut stmt = conn
        .execute(&paginated_sql, ())
        .context("Paginated query failed")?
        .ok_or_else(|| anyhow::anyhow!("No result set"))?;

    let mut buf = TextRowSet::for_cursor(page_size, &mut stmt, Some(4096))
        .context("Buffer allocation failed")?;
    let mut cursor = stmt.bind_buffer(&mut buf).context("Buffer bind failed")?;

    let mut rows = Vec::new();
    while let Some(batch) = cursor.fetch().context("Fetch failed")? {
        for row_idx in 0..batch.num_rows() {
            let mut row = Vec::with_capacity(num_cols);
            for col_idx in 0..num_cols {
                row.push(col_str_q(&batch, col_idx, row_idx));
            }
            rows.push(row);
        }
    }

    Ok((rows, total_count))
}

fn build_count_query(sql: &str) -> String {
    let upper = sql.to_uppercase();
    if let Some(from_pos) = upper.find(" FROM ") {
        format!("SELECT COUNT(*) {}", &sql[from_pos..])
    } else {
        format!("SELECT COUNT(*) FROM ({sql})")
    }
}

fn build_paginated_query(sql: &str, offset: usize, limit: usize) -> String {
    let upper = sql.to_uppercase();
    if let Some(select_end) = upper.find("SELECT ") {
        let after_select = select_end + 7;
        format!(
            "SELECT SKIP {offset} FIRST {limit} {}",
            &sql[after_select..]
        )
    } else {
        sql.to_string()
    }
}
