use serde::{Deserialize, Serialize};

// ── YAML Config Types ───────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryLinksConfig {
    pub links: Vec<QueryLink>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryLink {
    pub name: String,
    pub slug: String,
    pub master: MasterConfig,
    pub detail: Option<DetailConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterConfig {
    pub query: String,
    pub columns: Vec<ColumnDef>,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page_size() -> usize {
    50
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetailConfig {
    pub query: String,
    pub id_field: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColumnDef {
    pub field: String,
    pub label: String,
    #[serde(default)]
    pub link_to_detail: bool,
}

// ── Response Types ──────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MasterResponse {
    pub rows: Vec<Vec<String>>,
    pub columns: Vec<ColumnDef>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub link_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetailResponse {
    pub rows: Vec<Vec<String>>,
    pub columns: Vec<ColumnDef>,
    pub id_value: String,
    pub link_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSession {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
    pub authenticated_at: i64,
}

/// A participant record from the legacy Informix database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantRow {
    pub part_no: i32,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub gender: Option<String>,
    pub race_code: Option<String>,
    pub active: Option<String>,
    pub date_added: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticipantsResponse {
    pub participants: Vec<ParticipantRow>,
    pub count: usize,
}

/// A pool member — participant status within a pool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolMemberRow {
    pub pm_id: i32,
    pub pool_no: i32,
    pub part_no: i32,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub status: i32,
    pub rand_nbr: Option<i32>,
    pub responded: Option<String>,
}

/// A pool (draw group for a specific show).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolRow {
    pub pool_no: i32,
    pub show_no: Option<i32>,
    pub ret_date: Option<String>,
    pub div_code: Option<String>,
    pub office: Option<String>,
    pub capacity: Option<i32>,
    pub member_count: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolsResponse {
    pub pools: Vec<PoolRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolMembersResponse {
    pub members: Vec<PoolMemberRow>,
    pub count: usize,
    pub pool_no: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub server_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ── Pool Staff / Contacts ────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaffOption {
    pub co_code: String,
    pub co_translation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolStaffRow {
    pub ct_name: String,
    pub ct_type: String,
    pub schedule_count: i32,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
    pub has_codes_entry: bool,
    pub codes_options: Vec<StaffOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolStaffResponse {
    pub rows: Vec<PoolStaffRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplaceStaffParams {
    pub old_name: String,
    pub new_name: String,
    pub ct_type: String,
}

// ── Tasks ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskRow {
    pub id: String,
    pub description: String,
    pub task_type: String,
    pub status: String,
    pub result_summary: Option<String>,
    pub error_detail: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TasksResponse {
    pub tasks: Vec<TaskRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartTaskResponse {
    pub task_id: String,
    pub message: String,
}

// ── Phase 2: Dashboard & Operational ───────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardStatus {
    pub bad_show_codes: i64,
    pub blank_questionnaires: i64,
    pub portal_lockouts: i64,
    pub informix_sync_pending: i64,
    pub informix_sync_failed: i64,
    pub failed_tasks: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BadShowCodeRow {
    pub pm_id: i32,
    pub pool_no: i32,
    pub part_no: i32,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub bad_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BadShowCodesResponse {
    pub rows: Vec<BadShowCodeRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixShowCodeParams {
    pub pool_no: i32,
    pub new_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlankQQRow {
    pub pm_id: i32,
    pub pool_no: i32,
    pub part_no: i32,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub ret_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlankQQResponse {
    pub rows: Vec<BlankQQRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResetQQParams {
    pub pm_id: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortalLockoutRow {
    pub part_no: i32,
    pub fname: Option<String>,
    pub lname: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortalLockoutsResponse {
    pub rows: Vec<PortalLockoutRow>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnlockParams {
    pub part_no: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShowTypeRow {
    pub st_code: String,
    pub st_description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShowTypesResponse {
    pub rows: Vec<ShowTypeRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionResponse {
    pub ok: bool,
    pub message: String,
}

// ── Phase 5: Reviews ─────────────────────────────────────

/// A row in the admin review queue (Informix review_record + participant join).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminReviewRow {
    pub rr_id: i32,
    pub part_no: i32,
    pub pool_no: i32,
    pub part_key: String,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub review_type: String,
    pub status: String,
    pub admin_notes: Option<String>,
    pub submitted_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdminReviewQueue {
    pub rows: Vec<AdminReviewRow>,
    pub count: usize,
}

/// A row in the CEO queue (PostgreSQL status_reviews, names joined from Informix).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CeoReviewRow {
    pub id: String,
    pub part_no: String,
    pub pool_no: String,
    pub part_key: String,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub review_type: String,
    pub admin_notes: Option<String>,
    pub sent_to_ceo_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CeoReviewQueue {
    pub rows: Vec<CeoReviewRow>,
    pub count: usize,
    pub maintenance: bool,
}

/// Full detail view for both admin prep and CEO decision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewDetail {
    // Participant (Informix)
    pub part_no: i32,
    pub pool_no: i32,
    pub part_key: String,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub addr: Option<String>,
    pub city: Option<String>,
    pub state_code: Option<String>,
    pub zip: Option<String>,
    pub email: Option<String>,
    pub gender: Option<String>,
    pub race_code: Option<String>,
    pub active: Option<String>,
    // Pool (Informix)
    pub pool_div_code: Option<String>,
    pub pool_ret_date: Option<String>,
    // Pool member (Informix)
    pub pm_id: i32,
    pub member_status: i32,
    // Informix review_record
    pub review_type: String,
    pub ifx_status: String,
    pub admin_notes: Option<String>,
    pub submitted_date: Option<String>,
    // PostgreSQL status_reviews (None if not yet sent to CEO)
    pub pg_status: Option<String>,
    pub ceo_notes: Option<String>,
    pub decision: Option<String>,
    pub sent_to_ceo_at: Option<String>,
    pub decided_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SendToCeoParams {
    pub part_key: String,
    pub admin_notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CeoDecideParams {
    pub part_key: String,
    /// requalify | disqualify | permanent_excuse | temporary_excuse | send_back
    pub action: String,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecideResponse {
    pub ok: bool,
    pub message: String,
    pub was_duplicate: bool,
    pub status: String,
    pub decision: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewHistoryEntry {
    pub id: String,
    pub part_no: String,
    pub review_type: String,
    pub action: String,
    pub actor_email: Option<String>,
    pub notes: Option<String>,
    pub acted_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewHistoryResponse {
    pub entries: Vec<ReviewHistoryEntry>,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingCountsResponse {
    pub excuse_pending: i64,
    pub disqualify_pending: i64,
    pub ceo_queue: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CeoReviewStateResponse {
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetCeoStateParams {
    pub state: String,
}

/// A row in the unified review queue (PostgreSQL status_reviews + Informix names).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedReviewRow {
    pub id: String,
    pub part_no: String,
    pub pool_no: String,
    pub part_key: String,
    pub fname: Option<String>,
    pub lname: Option<String>,
    pub review_type: String,
    /// pending_admin | pending_ceo | completed | sent_back
    pub status: String,
    pub admin_notes: Option<String>,
    pub ceo_notes: Option<String>,
    pub decision: Option<String>,
    pub sent_to_ceo_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedReviewQueue {
    pub rows: Vec<UnifiedReviewRow>,
    pub count: usize,
    pub maintenance: bool,
    pub show_notes: bool,
    pub show_send_back: bool,
}

// ── Tickets ──────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketRow {
    pub id: String,
    pub task_id: Option<String>,
    pub status: String,
    pub description: String,
    pub admin_notes: Option<String>,
    pub user_email: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TicketsResponse {
    pub tickets: Vec<TicketRow>,
    pub count: usize,
}
