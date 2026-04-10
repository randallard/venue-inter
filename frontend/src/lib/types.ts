export interface UserSession {
	sub: string;
	email: string | null;
	name: string | null;
	groups: string[];
	authenticated_at: number;
}

// ── YAML Config Types ───────────────────────────────────────

export interface QueryLink {
	name: string;
	slug: string;
}

export interface ColumnDef {
	field: string;
	label: string;
	link_to_detail: boolean;
}

// ── Response Types ──────────────────────────────────────────

export interface MasterResponse {
	rows: string[][];
	columns: ColumnDef[];
	total_count: number;
	page: number;
	page_size: number;
	link_name: string;
}

export interface DetailResponse {
	rows: string[][];
	columns: ColumnDef[];
	id_value: string;
	link_name: string;
}

export interface ErrorResponse {
	error: string;
}

// ── Venue Domain Types ──────────────────────────────────────

export interface ParticipantRow {
	part_no: number;
	fname: string | null;
	lname: string | null;
	city: string | null;
	state: string | null;
	gender: string | null;
	race_code: string | null;
	active: string | null;
	date_added: string | null;
}

export interface PoolRow {
	pool_no: number;
	show_no: number | null;
	ret_date: string | null;
	div_code: string | null;
	office: string | null;
	capacity: number | null;
	member_count: number;
}

export interface PoolMemberRow {
	pm_id: number;
	pool_no: number;
	part_no: number;
	fname: string | null;
	lname: string | null;
	status: number;
	rand_nbr: number | null;
	responded: string | null;
}

export interface PoolStaffRow {
	sr_name: string;
	sr_type: string;
}

export interface ReplaceStaffParams {
	old_name: string;
	new_name: string;
	ct_type: string;
}

// ── Phase 2: Dashboard & Operational ───────────────────────

export interface DashboardStatus {
	bad_show_codes: number;
	blank_questionnaires: number;
	portal_lockouts: number;
	informix_sync_pending: number;
	informix_sync_failed: number;
}

export interface BadShowCodeRow {
	pm_id: number;
	pool_no: number;
	part_no: number;
	fname: string | null;
	lname: string | null;
	bad_code: string | null;
}

export interface BadShowCodesResponse {
	rows: BadShowCodeRow[];
	count: number;
}

export interface BlankQQRow {
	pm_id: number;
	pool_no: number;
	part_no: number;
	fname: string | null;
	lname: string | null;
	ret_date: string | null;
}

export interface BlankQQResponse {
	rows: BlankQQRow[];
	count: number;
}

export interface PortalLockoutRow {
	part_no: number;
	fname: string | null;
	lname: string | null;
}

export interface PortalLockoutsResponse {
	rows: PortalLockoutRow[];
	count: number;
}

export interface ShowTypeRow {
	st_code: string;
	st_description: string;
}

export interface ShowTypesResponse {
	rows: ShowTypeRow[];
}

export interface ActionResponse {
	ok: boolean;
	message: string;
}

// ── Review Types (Phase 5) ──────────────────────────────────

export interface AdminReviewRow {
	rr_id: number;
	part_no: number;
	pool_no: number;
	part_key: string;
	fname: string | null;
	lname: string | null;
	review_type: string;
	status: string;
	admin_notes: string | null;
	submitted_date: string | null;
}

export interface AdminReviewQueue {
	rows: AdminReviewRow[];
	count: number;
}

export interface CeoReviewRow {
	id: string;
	part_no: string;
	pool_no: string;
	part_key: string;
	fname: string | null;
	lname: string | null;
	review_type: string;
	admin_notes: string | null;
	sent_to_ceo_at: string | null;
}

export interface CeoReviewQueue {
	rows: CeoReviewRow[];
	count: number;
	maintenance: boolean;
}

export interface ReviewDetail {
	part_no: number;
	pool_no: number;
	part_key: string;
	fname: string | null;
	lname: string | null;
	addr: string | null;
	city: string | null;
	state_code: string | null;
	zip: string | null;
	email: string | null;
	gender: string | null;
	race_code: string | null;
	active: string | null;
	pool_div_code: string | null;
	pool_ret_date: string | null;
	pm_id: number;
	member_status: number;
	review_type: string;
	ifx_status: string;
	admin_notes: string | null;
	submitted_date: string | null;
	pg_status: string | null;
	ceo_notes: string | null;
	decision: string | null;
	sent_to_ceo_at: string | null;
	decided_at: string | null;
}

export interface DecideResponse {
	ok: boolean;
	message: string;
	was_duplicate: boolean;
	status: string;
	decision: string | null;
}

export interface ReviewHistoryEntry {
	id: string;
	part_no: string;
	review_type: string;
	action: string;
	actor_email: string | null;
	notes: string | null;
	acted_at: string;
}

export interface ReviewHistoryResponse {
	entries: ReviewHistoryEntry[];
	count: number;
}

export interface PendingCountsResponse {
	excuse_pending: number;
	disqualify_pending: number;
	ceo_queue: number;
}

export interface CeoReviewStateResponse {
	state: 'live' | 'maintenance';
}

// ── Sync Status Types ───────────────────────────────────────

export interface SyncStatusRow {
	part_no: string;
	pool_no: string;
	part_key: string;
	fname: string | null;
	lname: string | null;
	review_type: string;
	ifx_status: string | null;
	pg_status: string | null;
	pg_decision: string | null;
	sync_pending: number;
	sync_failed: number;
	sync_errors: string[];
	/** ok | active | syncing | warning | error | unprocessed */
	health: 'ok' | 'active' | 'syncing' | 'warning' | 'error' | 'unprocessed';
	health_reason: string | null;
}

export interface SyncStatusResponse {
	rows: SyncStatusRow[];
	total: number;
	error_count: number;
	warning_count: number;
	syncing_count: number;
	unprocessed_count: number;
}

export interface SyncOneResponse {
	processed: number;
	succeeded: number;
	failed: number;
	errors: string[];
}

