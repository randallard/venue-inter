use shared_types::TicketRow;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_ticket(
    pool: &PgPool,
    task_id: Uuid,
    user_sub: &str,
    user_email: Option<&str>,
    description: &str,
) -> Result<Uuid, sqlx::Error> {
    let row = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tickets (task_id, user_sub, user_email, description) \
         VALUES ($1, $2, $3, $4) RETURNING id"
    )
    .bind(task_id)
    .bind(user_sub)
    .bind(user_email)
    .bind(description)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn get_tickets_for_user(pool: &PgPool, user_sub: &str) -> Result<Vec<TicketRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PgTicketRow>(
        "SELECT id, task_id, status, description, admin_notes, user_email, created_at, updated_at \
         FROM tickets WHERE user_sub = $1 ORDER BY created_at DESC"
    )
    .bind(user_sub)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn get_all_tickets(pool: &PgPool) -> Result<Vec<TicketRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PgTicketRow>(
        "SELECT id, task_id, status, description, admin_notes, user_email, created_at, updated_at \
         FROM tickets ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

#[derive(sqlx::FromRow)]
struct PgTicketRow {
    id: Uuid,
    task_id: Option<Uuid>,
    status: String,
    description: String,
    admin_notes: Option<String>,
    user_email: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<PgTicketRow> for TicketRow {
    fn from(r: PgTicketRow) -> Self {
        TicketRow {
            id: r.id.to_string(),
            task_id: r.task_id.map(|u| u.to_string()),
            status: r.status,
            description: r.description,
            admin_notes: r.admin_notes,
            user_email: r.user_email,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}
