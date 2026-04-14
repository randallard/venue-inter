<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import {
		getUnifiedQueue,
		getReviewDetail,
		sendToCeo,
		ceoDecide,
		recallRecord,
		syncNow,
		listDocuments
	} from '$lib/api';
	import type { UnifiedReviewRow, ReviewDetail, UserSession, DocumentsResponse, DocumentMeta } from '$lib/types';

	let { data } = $props<{
		data: { user: UserSession | null; initialType: string; initialStatus: string | null };
	}>();

	const isCeo = $derived(data.user?.groups.includes('ceo-review') ?? false);

	// ── Filters ──────────────────────────────────────────────
	let typeFilter = $state(data.initialType ?? 'all');
	let statusFilter = $state(
		data.initialStatus ?? (isCeo ? 'pending_ceo' : 'pending_admin')
	);

	// ── Data ─────────────────────────────────────────────────
	type Case = {
		row: UnifiedReviewRow;
		detail: ReviewDetail | null;
		detailLoading: boolean;
		detailError: string | null;
		docs: DocumentsResponse | null;
		docsLoading: boolean;
		notes: string;
		acting: boolean;
		actionError: string | null;
	};

	let viewingDoc = $state<DocumentMeta | null>(null);

	let cases = $state<Case[]>([]);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let maintenance = $state(false);
	let showNotes = $state(true);
	let showSendBack = $state(true);
	let syncing = $state(false);
	let syncMsg = $state<string | null>(null);

	// Incremented on every load() call so in-flight detail/doc fetches from a
	// previous filter selection don't clobber the current results.
	let loadGen = 0;

	async function load() {
		const gen = ++loadGen;
		loading = true;
		loadError = null;
		try {
			const data = await getUnifiedQueue({
				type: typeFilter === 'all' ? undefined : typeFilter,
				status: statusFilter === 'all' ? undefined : statusFilter
			});
			if (loadGen !== gen) return;
			maintenance  = data.maintenance;
			showNotes    = data.show_notes;
			showSendBack = data.show_send_back;
			cases = data.rows.map((row) => ({
				row,
				detail: null,
				detailLoading: true,
				detailError: null,
				docs: null,
				docsLoading: row.status !== 'completed' && row.status !== 'sent_back',
				notes: row.admin_notes ?? '',
				acting: false,
				actionError: null
			}));
			data.rows.forEach((row, i) => {
				getReviewDetail(row.part_key)
					.then((detail) => {
						if (loadGen !== gen) return;
						cases[i] = { ...cases[i], detail, detailLoading: false, notes: detail.admin_notes ?? '' };
					})
					.catch((e) => {
						if (loadGen !== gen) return;
						cases[i] = {
							...cases[i],
							detailError: e instanceof Error ? e.message : 'Failed to load',
							detailLoading: false
						};
					});
				if (row.status !== 'completed' && row.status !== 'sent_back') {
					listDocuments(row.part_key)
						.then((docs) => {
							if (loadGen !== gen) return;
							cases[i] = { ...cases[i], docs, docsLoading: false };
						})
						.catch(() => { if (loadGen !== gen) return; cases[i] = { ...cases[i], docsLoading: false }; });
				}
			});
		} catch (e) {
			loadError = e instanceof Error ? e.message : 'Failed to load queue';
		} finally {
			if (loadGen === gen) loading = false;
		}
	}

	async function handleSyncNow() {
		syncing = true;
		syncMsg = null;
		try {
			const r = await syncNow();
			syncMsg = r.inserted > 0 ? `Synced ${r.inserted} new record${r.inserted !== 1 ? 's' : ''} from national system` : 'Already up to date';
			await load();
		} catch {
			syncMsg = 'Sync failed';
		} finally {
			syncing = false;
		}
	}

	// Admin: send pending_admin to CEO
	async function handleSendToCeo(i: number) {
		cases[i] = { ...cases[i], acting: true, actionError: null };
		try {
			const r = await sendToCeo({ part_key: cases[i].row.part_key, admin_notes: cases[i].notes || null });
			if (r.ok) {
				cases[i] = { ...cases[i], acting: false, row: { ...cases[i].row, status: 'pending_ceo', admin_notes: cases[i].notes || null } };
				if (statusFilter === 'pending_admin') await load();
			} else {
				cases[i] = { ...cases[i], acting: false, actionError: r.message };
			}
		} catch (e) {
			cases[i] = { ...cases[i], acting: false, actionError: e instanceof Error ? e.message : 'Request failed' };
		}
	}

	// Admin: recall pending_ceo back to admin
	async function handleRecall(i: number) {
		cases[i] = { ...cases[i], acting: true, actionError: null };
		try {
			const r = await recallRecord(cases[i].row.part_key);
			if (r.ok) {
				cases[i] = { ...cases[i], acting: false, row: { ...cases[i].row, status: 'pending_admin' } };
				if (statusFilter === 'pending_ceo') await load();
			} else {
				cases[i] = { ...cases[i], acting: false, actionError: r.message };
			}
		} catch (e) {
			cases[i] = { ...cases[i], acting: false, actionError: e instanceof Error ? e.message : 'Request failed' };
		}
	}

	// CEO: full decision
	async function handleDecide(i: number, action: string) {
		const c = cases[i];
		if (showNotes && !c.notes.trim()) {
			cases[i] = { ...cases[i], actionError: 'Notes are required before making a decision.' };
			return;
		}
		cases[i] = { ...cases[i], acting: true, actionError: null };
		const part_key = c.row.part_key;
		try {
			const res = await ceoDecide({ part_key, action, notes: c.notes });
			if (res.ok) {
				cases[i] = {
					...cases[i],
					acting: false,
					row: { ...cases[i].row, status: res.status, decision: res.decision ?? null },
					detail: cases[i].detail
						? { ...cases[i].detail!, pg_status: res.status, decision: res.decision ?? null, decided_at: new Date().toISOString() }
						: null
				};
			} else {
				cases[i] = { ...cases[i], acting: false, actionError: res.message };
			}
		} catch (e) {
			cases[i] = { ...cases[i], acting: false, actionError: e instanceof Error ? e.message : 'Request failed' };
		}
	}

	const memberStatusLabel: Record<number, string> = {
		1: 'In Pool', 2: 'Qualified', 5: 'Perm Excuse', 6: 'Disqualified', 7: 'Temp Excuse'
	};

	function formatDate(iso: string | null | undefined) {
		if (!iso) return '—';
		return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}

	function decisionLabel(d: string | null, status: string) {
		if (status === 'sent_back') return 'Sent back to admin';
		if (status === 'recalled') return 'Recalled by admin';
		if (!d) return '—';
		return d.replace(/_/g, ' ');
	}

	const pendingCount = $derived(
		cases.filter((c) => c.row.status === 'pending_admin' || c.row.status === 'pending_ceo').length
	);

	$effect(() => { typeFilter; statusFilter; load(); });
	onMount(() => {});
</script>

<div class="page-wrap">
	<div class="page-header">
		<div>
			<h1>Review Queue</h1>
			{#if !loading && !loadError}
				<p class="subhead">
					{#if cases.length === 0}
						No records match current filters.
					{:else}
						{pendingCount} active · {cases.length} total shown
					{/if}
				</p>
			{/if}
		</div>
		<div class="header-actions">
			{#if !isCeo}
				<button class="btn btn-secondary btn-sm" onclick={handleSyncNow} disabled={syncing}>
					{syncing ? 'Syncing…' : 'Sync now'}
				</button>
			{/if}
			<button class="btn btn-secondary btn-sm" onclick={load} disabled={loading}>Refresh</button>
		</div>
	</div>

	{#if syncMsg}
		<p class="sync-msg">{syncMsg}</p>
	{/if}

	<!-- Filters (hidden for CEO — they only ever see their own queue) -->
	{#if !isCeo}
	<div class="filter-bar">
		<div class="filter-group">
			<span class="filter-label">Type</span>
			{#each ['all', 'excuse', 'disqualify'] as t}
				<button
					class="chip {typeFilter === t ? 'chip-active' : ''}"
					onclick={() => (typeFilter = t)}
				>{t === 'all' ? 'All' : t.charAt(0).toUpperCase() + t.slice(1)}</button>
			{/each}
		</div>
		<div class="filter-group">
			<span class="filter-label">Status</span>
			{#each [
				{ val: 'pending_admin', label: 'Admin Queue' },
				{ val: 'pending_ceo',   label: 'CEO Queue' },
				{ val: 'all',           label: 'All' }
			] as s}
				<button
					class="chip {statusFilter === s.val ? 'chip-active' : ''}"
					onclick={() => (statusFilter = s.val)}
				>{s.label}</button>
			{/each}
		</div>
	</div>
	{/if}

	{#if maintenance && isCeo}
		<div class="maintenance-banner">
			Queue in maintenance mode — decisions are paused.
		</div>
	{/if}

	{#if loading}
		<p class="text-muted loading-msg">Loading…</p>
	{:else if loadError}
		<p class="error">{loadError}</p>
	{:else if cases.length === 0}
		<div class="card empty-card">
			<span class="ok-icon">✓</span>
			<p>No records match current filters.</p>
		</div>
	{:else}
		<div class="case-list">
			{#each cases as c, i}
				{@const decided = c.row.status === 'completed' || c.row.status === 'sent_back'}

				{#if decided}
					<div class="case-decided">
						<div class="decided-info">
							<span class="decided-name">{c.row.lname ?? '—'}, {c.row.fname ?? '—'}</span>
							<span class="decided-meta">
								<span class="type-badge type-{c.row.review_type}">{c.row.review_type}</span>
								Pool #{c.row.pool_no}
							</span>
						</div>
						<div class="decided-result">
							<span class="decided-label {c.row.status === 'sent_back' ? 'sent-back' : 'resolved'}">
								{decisionLabel(c.row.decision, c.row.status)}
							</span>
						</div>
					</div>
				{:else}
					<div class="case-card" id="case-{c.row.part_key}">
						<div class="case-head">
							<div class="case-title">
								<span class="case-name">{c.row.lname ?? '—'}, {c.row.fname ?? '—'}</span>
								<span class="case-meta-head">
									<span class="type-badge type-{c.row.review_type}">{c.row.review_type}</span>
									<span class="status-chip status-{c.row.status}">
										{c.row.status === 'pending_admin' ? 'Admin Queue' : 'CEO Queue'}
									</span>
									Part #{c.row.part_no} · Pool #{c.row.pool_no}
									{#if c.detail?.pool_ret_date} · {c.detail.pool_ret_date}{/if}
									{#if c.row.sent_to_ceo_at} · Sent {formatDate(c.row.sent_to_ceo_at)}{/if}
								</span>
							</div>
						</div>

						{#if c.detailLoading}
							<p class="text-muted case-loading">Loading details…</p>
						{:else if c.detailError}
							<p class="error case-loading">{c.detailError}</p>
						{:else if c.detail}
							<div class="case-body">
								<!-- Info column -->
								<div class="info-col">
									<div class="info-section">
										<div class="info-label">Participant</div>
										<table class="info-table">
											<tbody>
												<tr>
													<td>Address</td>
													<td>{[c.detail.addr, c.detail.city, c.detail.state_code, c.detail.zip].filter(Boolean).join(', ') || '—'}</td>
												</tr>
												<tr><td>Email</td><td>{c.detail.email ?? '—'}</td></tr>
												<tr><td>Gender</td><td>{c.detail.gender ?? '—'}</td></tr>
												<tr><td>Race</td><td>{c.detail.race_code ?? '—'}</td></tr>
											</tbody>
										</table>
									</div>
									<div class="info-section">
										<div class="info-label">Pool Status</div>
										<table class="info-table">
											<tbody>
												<tr>
													<td>Status</td>
													<td><span class="status-pill">{memberStatusLabel[c.detail.member_status] ?? c.detail.member_status}</span></td>
												</tr>
												<tr><td>Submitted</td><td>{c.detail.submitted_date ?? '—'}</td></tr>
											</tbody>
										</table>
									</div>

									{#if showNotes && c.row.status === 'pending_ceo' && c.row.admin_notes}
										<div class="info-section">
											<div class="info-label">Admin Notes</div>
											<p class="notes-text">{c.row.admin_notes}</p>
										</div>
									{/if}

									{#if showNotes && c.row.ceo_notes}
										<div class="info-section">
											<div class="info-label">CEO Notes</div>
											<p class="notes-text">{c.row.ceo_notes}</p>
										</div>
									{/if}

									<!-- Documents -->
									<div class="info-section">
										<div class="info-label">Documents</div>
										{#if c.docsLoading}
											<p class="text-muted" style="font-size:0.85rem">Loading…</p>
										{:else if !c.docs || c.docs.documents.length === 0}
											<p class="text-muted" style="font-size:0.85rem">No documents on file.</p>
										{:else}
											{#if c.docs.scan_code}
												<p class="scan-badge">
													{c.docs.scan_code === 'web' ? 'Submitted online' : `Scan batch: ${c.docs.scan_code}`}
												</p>
											{/if}
											<ul class="doc-list">
												{#each c.docs.documents as doc}
													<li class="doc-row">
														<span class="doc-name">{doc.file_name}</span>
														<button class="btn-view" onclick={() => viewingDoc = doc}>View</button>
													</li>
												{/each}
											</ul>
										{/if}
									</div>
								</div>

								<!-- Action column -->
								<div class="decision-col">
									{#if c.row.status === 'pending_admin' && !isCeo}
										<!-- Admin: send to CEO -->
										{#if showNotes}
											<div class="info-label">Admin Notes</div>
											<label class="sr-only" for="notes-{c.row.part_key}">Notes</label>
											<textarea
												id="notes-{c.row.part_key}"
												class="notes-area"
												bind:value={cases[i].notes}
												placeholder="Optional — document your review"
												rows="4"
												disabled={c.acting}
											></textarea>
										{/if}
										{#if c.actionError}
											<p class="form-error">{c.actionError}</p>
										{/if}
										<button
											class="btn-action btn-send"
											onclick={() => handleSendToCeo(i)}
											disabled={c.acting}
										>{c.acting ? 'Sending…' : 'Send to CEO'}</button>

									{:else if c.row.status === 'pending_ceo' && !isCeo}
										<!-- Admin: recall -->
										{#if c.actionError}
											<p class="form-error">{c.actionError}</p>
										{/if}
										{#if showSendBack}
											<button
												class="btn-action btn-recall"
												onclick={() => handleRecall(i)}
												disabled={c.acting}
											>{c.acting ? 'Recalling…' : 'Recall to Admin'}</button>
										{/if}

									{:else if c.row.status === 'pending_ceo' && isCeo}
										<!-- CEO: full decision panel -->
										{#if showNotes}
											<div class="info-label">Your Decision</div>
											<label class="sr-only" for="notes-{c.row.part_key}">Notes</label>
											<textarea
												id="notes-{c.row.part_key}"
												class="notes-area"
												bind:value={cases[i].notes}
												placeholder="Required — document your reasoning"
												rows="4"
												disabled={c.acting}
											></textarea>
										{/if}
										{#if c.actionError}
											<p class="form-error">{c.actionError}</p>
										{/if}
										<div class="decision-btns">
											<button class="btn-decide btn-requalify" onclick={() => handleDecide(i, 'requalify')} disabled={c.acting}>Re-qualify</button>
											<button class="btn-decide btn-disqualify" onclick={() => handleDecide(i, 'disqualify')} disabled={c.acting}>Disqualify</button>
											<button class="btn-decide btn-perm" onclick={() => handleDecide(i, 'permanent_excuse')} disabled={c.acting}>Perm Excuse</button>
											<button class="btn-decide btn-temp" onclick={() => handleDecide(i, 'temporary_excuse')} disabled={c.acting}>Temp Excuse</button>
											{#if showSendBack}
												<button class="btn-decide btn-sendback" onclick={() => handleDecide(i, 'send_back')} disabled={c.acting}>Send Back</button>
											{/if}
										</div>
										{#if c.acting}
											<p class="deciding-msg text-muted">Recording decision…</p>
										{/if}
									{/if}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.page-wrap {
		max-width: 1100px;
		margin: 0 auto;
		padding: 1.5rem 2rem 4rem;
	}
	.page-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		margin-bottom: 1rem;
	}
	.page-header h1 { margin: 0 0 0.2rem; }
	.subhead { margin: 0; color: var(--text-muted); font-size: 0.92rem; }
	.header-actions { display: flex; gap: 0.5rem; }
	.loading-msg { margin-top: 2rem; }
	.sync-msg { font-size: 0.85rem; color: var(--text-muted); margin: 0 0 0.75rem; }

	/* Filters */
	.filter-bar {
		display: flex;
		flex-wrap: wrap;
		gap: 1rem;
		margin-bottom: 1.25rem;
		padding-bottom: 1rem;
		border-bottom: 1px solid var(--border);
	}
	.filter-group { display: flex; align-items: center; gap: 0.35rem; }
	.filter-label {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-muted);
		margin-right: 0.15rem;
	}
	.chip {
		padding: 0.25rem 0.7rem;
		border: 1px solid var(--border);
		border-radius: 999px;
		background: var(--surface);
		color: var(--text-muted);
		font-size: 0.82rem;
		cursor: pointer;
		transition: background 0.12s, color 0.12s, border-color 0.12s;
	}
	.chip:hover { border-color: var(--gold); color: var(--text); }
	.chip-active { background: var(--navy); color: #fff; border-color: var(--navy); }

	.maintenance-banner {
		background: #fef9c3;
		color: #854d0e;
		border: 1px solid #fde68a;
		border-radius: var(--radius);
		padding: 0.65rem 1rem;
		font-size: 0.9rem;
		font-weight: 600;
		margin-bottom: 1rem;
	}

	.empty-card {
		text-align: center;
		padding: 3rem;
		color: var(--text-muted);
	}
	.ok-icon { font-size: 2rem; color: var(--green); display: block; margin-bottom: 0.5rem; }

	/* Case list */
	.case-list { display: flex; flex-direction: column; gap: 1rem; }

	.case-card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
	}
	.case-head {
		background: var(--navy);
		padding: 0.85rem 1.25rem;
	}
	.case-title { display: flex; flex-direction: column; gap: 0.25rem; }
	.case-name { font-size: 1.15rem; font-weight: 700; color: #fff; }
	.case-meta-head {
		font-size: 0.85rem;
		color: rgba(255,255,255,0.7);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.case-body {
		display: grid;
		grid-template-columns: 1fr 380px;
	}
	@media (max-width: 860px) { .case-body { grid-template-columns: 1fr; } }

	.case-loading { padding: 1rem 1.25rem; }

	.info-col {
		padding: 1.1rem 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		border-right: 1px solid var(--border);
	}
	@media (max-width: 860px) { .info-col { border-right: none; border-bottom: 1px solid var(--border); } }

	.info-section {}
	.info-label {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-muted);
		margin-bottom: 0.4rem;
	}
	.info-table { width: 100%; border-collapse: collapse; font-size: 0.88rem; }
	.info-table td { padding: 0.2rem 0.4rem 0.2rem 0; vertical-align: top; }
	.info-table td:first-child { color: var(--text-muted); white-space: nowrap; padding-right: 0.75rem; width: 80px; }

	.status-pill {
		display: inline-block;
		padding: 0.1rem 0.5rem;
		background: var(--gold-light, #fef9c3);
		color: var(--gold-dark, #854d0e);
		border-radius: 3px;
		font-size: 0.8rem;
		font-weight: 600;
	}
	.notes-text { font-size: 0.9rem; color: var(--text-muted); margin: 0; line-height: 1.5; }

	/* Action column */
	.decision-col {
		padding: 1.1rem 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0,0,0,0); }
	.notes-area {
		width: 100%;
		padding: 0.5rem 0.65rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-family: inherit;
		font-size: 0.88rem;
		resize: vertical;
		background: var(--surface);
		box-sizing: border-box;
	}
	.notes-area:focus { outline: none; border-color: var(--gold); box-shadow: 0 0 0 3px rgba(181,152,90,0.15); }
	.form-error { color: var(--red); font-size: 0.85rem; font-weight: 600; margin: 0; }

	.btn-action {
		padding: 0.55rem 1rem;
		font-size: 0.9rem;
		font-weight: 700;
		border-radius: var(--radius-sm);
		border: none;
		cursor: pointer;
		transition: opacity 0.15s;
	}
	.btn-action:disabled { opacity: 0.4; cursor: not-allowed; }
	.btn-send { background: var(--navy); color: #fff; }
	.btn-send:not(:disabled):hover { background: var(--navy-light, #1e3a5f); }
	.btn-recall { background: #f1f5f9; color: #475569; border: 1px solid var(--border); }
	.btn-recall:not(:disabled):hover { background: #e2e8f0; }

	/* CEO decision buttons */
	.decision-btns { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; }
	.btn-decide {
		padding: 0.55rem 0.5rem;
		font-size: 0.85rem;
		font-weight: 700;
		border-radius: var(--radius-sm);
		border: none;
		cursor: pointer;
		transition: opacity 0.15s;
	}
	.btn-decide:disabled { opacity: 0.4; cursor: not-allowed; }
	.btn-requalify  { background: #dcfce7; color: #166534; }
	.btn-requalify:not(:disabled):hover  { background: #bbf7d0; }
	.btn-disqualify { background: #fef2f2; color: #991b1b; }
	.btn-disqualify:not(:disabled):hover { background: #fee2e2; }
	.btn-perm       { background: #f1f5f9; color: #475569; }
	.btn-perm:not(:disabled):hover       { background: #e2e8f0; }
	.btn-temp       { background: #fef9c3; color: #854d0e; }
	.btn-temp:not(:disabled):hover       { background: #fef08a; }
	.btn-sendback   { grid-column: 1 / -1; background: var(--navy); color: #fff; }
	.btn-sendback:not(:disabled):hover   { background: var(--navy-light, #1e3a5f); }
	.deciding-msg { font-size: 0.85rem; text-align: center; margin: 0; }

	/* Status chips in header */
	.status-chip {
		display: inline-block;
		padding: 0.1rem 0.45rem;
		border-radius: 3px;
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.status-pending_admin { background: #fef9c3; color: #854d0e; }
	.status-pending_ceo   { background: #e0e7ff; color: #3730a3; }

	/* Decided compact row */
	.case-decided {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.65rem 1.1rem;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		opacity: 0.7;
		gap: 1rem;
	}
	.decided-info { display: flex; align-items: center; gap: 0.75rem; font-size: 0.9rem; }
	.decided-name { font-weight: 600; }
	.decided-meta { display: flex; align-items: center; gap: 0.4rem; color: var(--text-muted); font-size: 0.85rem; }
	.decided-result { display: flex; align-items: center; gap: 0.75rem; flex-shrink: 0; }
	.decided-label { font-size: 0.82rem; font-weight: 700; text-transform: capitalize; padding: 0.15rem 0.6rem; border-radius: 3px; }
	.decided-label.resolved  { background: #dcfce7; color: #166534; }
	.decided-label.sent-back { background: #f1f5f9; color: #475569; }

	/* Type badge */
	.type-badge {
		display: inline-block;
		padding: 0.1rem 0.45rem;
		border-radius: 3px;
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: capitalize;
	}
	.type-excuse     { background: #fef9c3; color: #854d0e; }
	.type-disqualify { background: #fef2f2; color: #991b1b; }

	.btn-sm { padding: 0.3rem 0.75rem; font-size: 0.82rem; }

	/* Documents */
	.docs-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.4rem;
	}
	.btn-refresh {
		font-size: 0.78rem;
		padding: 0.15rem 0.5rem;
		background: none;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		cursor: pointer;
		color: var(--text-muted);
	}
	.btn-refresh:hover { border-color: var(--navy); color: var(--navy); }
	.scan-badge { font-size: 0.8rem; color: var(--gold); font-weight: 600; margin: 0 0 0.4rem; }
	.doc-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; }
	.doc-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.3rem 0;
		border-bottom: 1px solid var(--border);
	}
	.doc-row:last-child { border-bottom: none; }
	.doc-name { font-size: 0.84rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.btn-view {
		flex-shrink: 0;
		font-size: 0.78rem;
		padding: 0.18rem 0.55rem;
		background: var(--navy);
		color: #fff;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	.btn-view:hover { background: var(--navy-light, #1e3a5f); }
	.doc-status { flex-shrink: 0; font-size: 0.78rem; font-weight: 600; }
	.doc-status.pending { color: var(--gold); }
	.doc-status.failed  { color: var(--red); }

	/* Image viewer modal */
	:global(.doc-overlay) {
		position: fixed;
		inset: 0;
		background: rgba(0,0,0,0.75);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}
	:global(.doc-viewer) {
		background: #fff;
		border-radius: var(--radius);
		overflow: hidden;
		max-width: 90vw;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 8px 32px rgba(0,0,0,0.4);
	}
	:global(.doc-viewer-header) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.6rem 1rem;
		background: var(--navy);
		color: #fff;
	}
	:global(.doc-viewer-name) { font-size: 0.88rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	:global(.doc-close) { background: none; border: none; color: rgba(255,255,255,0.8); font-size: 1.1rem; cursor: pointer; padding: 0 0.25rem; flex-shrink: 0; }
	:global(.doc-close:hover) { color: #fff; }
	:global(.doc-img) { max-width: 85vw; max-height: calc(90vh - 44px); object-fit: contain; display: block; }
</style>

{#if viewingDoc}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="doc-overlay" onclick={() => viewingDoc = null}>
		<div class="doc-viewer" onclick={(e) => e.stopPropagation()}>
			<div class="doc-viewer-header">
				<span class="doc-viewer-name">{viewingDoc.file_name}</span>
				<button class="doc-close" onclick={() => viewingDoc = null}>✕</button>
			</div>
			<img src="/api/documents/view?path={encodeURIComponent(viewingDoc.webdav_path)}" alt={viewingDoc.file_name} class="doc-img" />
		</div>
	</div>
{/if}
