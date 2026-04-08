import type {
	UserSession,
	QueryLink,
	MasterResponse,
	DetailResponse,
	ErrorResponse,
	ParticipantRow,
	PoolRow,
	PoolMemberRow,
	PoolStaffRow,
	ReplaceStaffParams,
	DashboardStatus,
	BadShowCodesResponse,
	BlankQQResponse,
	PortalLockoutsResponse,
	ShowTypesResponse,
	ActionResponse,
	AdminReviewQueue,
	CeoReviewQueue,
	ReviewDetail,
	DecideResponse,
	ReviewHistoryResponse,
	PendingCountsResponse,
	CeoReviewStateResponse
} from './types';

class ApiError extends Error {
	status: number;
	constructor(status: number, message: string) {
		super(message);
		this.status = status;
	}
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
	const res = await fetch(path, {
		credentials: 'include',
		...init
	});

	if (res.status === 401) {
		window.location.href = '/auth/login?return_to=' + encodeURIComponent(window.location.pathname);
		throw new ApiError(401, 'Not authenticated');
	}

	if (!res.ok) {
		const body: ErrorResponse = await res.json().catch(() => ({ error: res.statusText }));
		throw new ApiError(res.status, body.error);
	}

	return res.json();
}

// ── Auth ────────────────────────────────────────────────────

export async function getCurrentUser(): Promise<UserSession> {
	return apiFetch('/api/current_user');
}

// ── Data Browser ────────────────────────────────────────────

export async function getQueryLinks(): Promise<QueryLink[]> {
	return apiFetch('/api/query_links');
}

export async function getMasterList(slug: string, page = 0, pageSize = 50): Promise<MasterResponse> {
	return apiFetch(`/api/queries/${slug}?page=${page}&page_size=${pageSize}`);
}

export async function getDetail(slug: string, id: string): Promise<DetailResponse> {
	return apiFetch(`/api/queries/${slug}/${id}`);
}

// ── Dashboard ───────────────────────────────────────────────

export async function getDashboardStatus(): Promise<DashboardStatus> {
	return apiFetch('/api/dashboard/status');
}

export async function getShowTypes(): Promise<ShowTypesResponse> {
	return apiFetch('/api/dashboard/show-types');
}

// ── Pool Operational ─────────────────────────────────────────

export async function getBadShowCodes(): Promise<BadShowCodesResponse> {
	return apiFetch('/api/pools/fix-show-codes');
}

export async function fixShowCode(params: { pool_no: number; new_code: string }): Promise<ActionResponse> {
	return apiFetch('/api/pools/fix-show-codes', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(params)
	});
}

export async function getBlankQuestionnaires(): Promise<BlankQQResponse> {
	return apiFetch('/api/pools/blank-questionnaires');
}

export async function resetQQ(pm_id: number): Promise<ActionResponse> {
	return apiFetch('/api/pools/reset-qq', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ pm_id })
	});
}

export async function getPortalLockouts(): Promise<PortalLockoutsResponse> {
	return apiFetch('/api/pools/lockouts');
}

export async function unlockParticipant(part_no: number): Promise<ActionResponse> {
	return apiFetch('/api/pools/unlock', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ part_no })
	});
}

// ── Participants ─────────────────────────────────────────────

export async function getParticipants(): Promise<ParticipantRow[]> {
	return apiFetch('/api/participants');
}

// ── Pools ────────────────────────────────────────────────────

export async function getPools(): Promise<PoolRow[]> {
	return apiFetch('/api/pools');
}

export async function getPoolMembers(poolNo: number): Promise<PoolMemberRow[]> {
	return apiFetch(`/api/pools/${poolNo}/members`);
}

// ── Pool Staff ───────────────────────────────────────────────

export async function getPoolStaff(): Promise<PoolStaffRow[]> {
	return apiFetch('/api/pool_staff');
}

export async function replaceStaff(params: ReplaceStaffParams): Promise<{ task_id: string }> {
	return apiFetch('/api/tasks/replace_staff', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(params)
	});
}

// ── Reviews (Phase 5) ────────────────────────────────────────

export async function getAdminReviewQueue(
	type: 'excuse' | 'disqualify'
): Promise<AdminReviewQueue> {
	return apiFetch(`/api/reviews/${type}/admin`);
}

export async function getReviewDetail(part_key: string): Promise<ReviewDetail> {
	return apiFetch(`/api/reviews/${part_key}`);
}

export async function sendToCeo(params: {
	part_key: string;
	admin_notes: string | null;
}): Promise<ActionResponse> {
	return apiFetch('/api/reviews/send-to-ceo', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(params)
	});
}

export async function getCeoQueue(): Promise<CeoReviewQueue> {
	return apiFetch('/api/reviews/ceo');
}

export async function ceoDecide(params: {
	part_key: string;
	action: string;
	notes: string;
}): Promise<DecideResponse> {
	return apiFetch('/api/reviews/ceo/decide', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(params)
	});
}

export async function getReviewHistory(part_no: number | string): Promise<ReviewHistoryResponse> {
	return apiFetch(`/api/reviews/records/${part_no}`);
}

export async function getPendingCounts(): Promise<PendingCountsResponse> {
	return apiFetch('/api/reviews/pending');
}

export async function getCeoReviewState(): Promise<CeoReviewStateResponse> {
	return apiFetch('/api/reviews/ceo-state');
}

export async function setCeoReviewState(state: 'live' | 'maintenance'): Promise<CeoReviewStateResponse> {
	return apiFetch('/api/reviews/ceo-state', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ state })
	});
}

export { ApiError };
