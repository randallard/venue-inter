use shared_types::TaskRow;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_task(
    pool: &PgPool,
    user_sub: &str,
    user_email: Option<&str>,
    description: &str,
    task_type: &str,
    task_params: &serde_json::Value,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tasks (user_sub, user_email, description, task_type, task_params) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id"
    )
    .bind(user_sub)
    .bind(user_email)
    .bind(description)
    .bind(task_type)
    .bind(task_params)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn update_task_status(
    pool: &PgPool,
    task_id: Uuid,
    status: &str,
    result_summary: Option<&str>,
    error_detail: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE tasks SET status = $1, result_summary = $2, error_detail = $3, updated_at = now() \
         WHERE id = $4"
    )
    .bind(status)
    .bind(result_summary)
    .bind(error_detail)
    .bind(task_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_tasks_for_user(pool: &PgPool, user_sub: &str) -> Result<Vec<TaskRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PgTaskRow>(
        "SELECT id, description, task_type, status, result_summary, error_detail, created_at, updated_at \
         FROM tasks WHERE user_sub = $1 ORDER BY created_at DESC"
    )
    .bind(user_sub)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn get_task_by_id(pool: &PgPool, task_id: Uuid) -> Result<Option<TaskRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, PgTaskRow>(
        "SELECT id, description, task_type, status, result_summary, error_detail, created_at, updated_at \
         FROM tasks WHERE id = $1"
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn get_task_user_info(pool: &PgPool, task_id: Uuid) -> Result<Option<(String, Option<String>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT user_sub, user_email FROM tasks WHERE id = $1"
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

#[derive(sqlx::FromRow)]
struct PgTaskRow {
    id: Uuid,
    description: String,
    task_type: String,
    status: String,
    result_summary: Option<String>,
    error_detail: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<PgTaskRow> for TaskRow {
    fn from(r: PgTaskRow) -> Self {
        TaskRow {
            id: r.id.to_string(),
            description: r.description,
            task_type: r.task_type,
            status: r.status,
            result_summary: r.result_summary,
            error_detail: r.error_detail,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}
