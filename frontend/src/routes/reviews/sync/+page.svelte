<script lang="ts">
	import { onMount } from 'svelte';
	import { getSyncStatus, syncOneRecord, lookupSyncRecord } from '$lib/api';
	import { ApiError } from '$lib/api';
	import type { SyncStatusResponse, SyncStatusRow } from '$lib/types';

	// ── Main listing ─────────────────────────────────────────

	let data = $state<SyncStatusResponse | null>(null);
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let filter = $state<string>('all');

	// ── Per-record sync ──────────────────────────────────────

	/** part_key → 'syncing' | 'done' | 'error:<msg>' */
	let syncState = $state<Record<string, string>>({});

	async function triggerSync(part_key: string) {
		syncState[part_key] = 'syncing';
		try {
			const r = await syncOneRecord(part_key);
			syncState[part_key] = r.failed > 0 ? `error:${r.errors[0] ?? 'failed'}` : 'done';
			// Refresh the full table so health badges update
			data = await getSyncStatus();
		} catch (e: unknown) {
			syncState[part_key] = `error:${e instanceof ApiError ? e.message : 'Request failed'}`;
		}
	}

	// ── Lookup ───────────────────────────────────────────────

	let lookupQuery = $state('');
	let lookupResult = $state<SyncStatusResponse | null>(null);
	let lookupLoading = $state(false);
	let lookupError = $state<string | null>(null);

	async function doLookup() {
		const q = lookupQuery.trim();
		if (!q) return;
		lookupLoading = true;
		lookupError = null;
		lookupResult = null;
		try {
			lookupResult = await lookupSyncRecord(q);
		} catch (e: unknown) {
			lookupError = e instanceof ApiError ? e.message : 'Lookup failed';
		} finally {
			lookupLoading = false;
		}
	}

	function onLookupKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') doLookup();
	}

	// ── Init ─────────────────────────────────────────────────

	onMount(async () => {
		try {
			data = await getSyncStatus();
		} catch (e: unknown) {
			loadError = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	});

	// ── Helpers ──────────────────────────────────────────────

	const healthLabel: Record<string, string> = {
		ok: 'OK',
		active: 'Active',
		syncing: 'Syncing',
		warning: 'Warning',
		error: 'Error',
		unprocessed: 'Unprocessed'
	};

	const pgStatusLabel: Record<string, string> = {
		pending_admin: 'Admin queue',
		pending_ceo: 'CEO queue',
		completed: 'Completed',
		sent_back: 'Sent back'
	};

	function displayName(row: SyncStatusRow): string {
		return [row.fname, row.lname].filter(Boolean).join(' ') || '—';
	}

	let visibleRows = $derived(
		data
			? filter === 'all'
				? data.rows
				: data.rows.filter((r) => r.health === filter)
			: []
	);

	function syncBtnLabel(part_key: string): string {
		const s = syncState[part_key];
		if (!s || s === 'done') return 'Sync';
		if (s === 'syncing') return 'Syncing…';
		if (s.startsWith('error:')) return 'Retry';
		return 'Sync';
	}

	function syncBtnTitle(part_key: string): string {
		const s = syncState[part_key];
		if (s?.startsWith('error:')) return s.slice(6);
		return 'Trigger immediate Informix sync for this record';
	}
</script>

<div class="container">
	<div class="page-header">
		<div>
			<h1>Sync Status</h1>
			<p>Cross-system view: Informix review records vs PostgreSQL workflow state vs sync queue</p>
		</div>
		<a class="btn btn-secondary" href="/reviews">← Reviews</a>
	</div>

	<!-- Lookup -->
	<div class="lookup-bar">
		<input
			class="lookup-input"
			type="text"
			placeholder="Look up any record — enter part_no or part_key (e.g. 12345 or 12345_678)"
			bind:value={lookupQuery}
			onkeydown={onLookupKeydown}
		/>
		<button class="btn btn-secondary" onclick={doLookup} disabled={lookupLoading || !lookupQuery.trim()}>
			{lookupLoading ? 'Looking up…' : 'Look up'}
		</button>
	</div>

	{#if lookupError}
		<div class="alert alert-error">{lookupError}</div>
	{/if}

	{#if lookupResult !== null}
		<div class="lookup-result">
			<div class="lookup-result-header">
				<strong>Lookup: {lookupQuery.trim()}</strong>
				{#if lookupResult.total === 0}
					<span class="text-muted"> — no records found in Informix or PostgreSQL</span>
				{:else}
					<span class="text-muted"> — {lookupResult.total} record{lookupResult.total !== 1 ? 's' : ''} found</span>
				{/if}
				<button class="clear-btn" onclick={() => (lookupResult = null)}>✕</button>
			</div>
			{#if lookupResult.total > 0}
				<SyncTable rows={lookupResult.rows} {syncState} {triggerSync} {displayName} {healthLabel} {pgStatusLabel} {syncBtnLabel} {syncBtnTitle} />
			{/if}
		</div>
	{/if}

	<!-- Main listing -->
	{#if loading}
		<p class="loading">Loading…</p>
	{:else if loadError}
		<div class="alert alert-error">{loadError}</div>
	{:else if data}
		<div class="summary-bar">
			<button class="summary-chip {filter === 'all' ? 'active' : ''}" onclick={() => (filter = 'all')}>
				All <span class="chip-count">{data.total}</span>
			</button>
			{#if data.error_count > 0}
				<button class="summary-chip chip-error {filter === 'error' ? 'active' : ''}" onclick={() => (filter = 'error')}>
					Errors <span class="chip-count">{data.error_count}</span>
				</button>
			{/if}
			{#if data.warning_count > 0}
				<button class="summary-chip chip-warning {filter === 'warning' ? 'active' : ''}" onclick={() => (filter = 'warning')}>
					Warnings <span class="chip-count">{data.warning_count}</span>
				</button>
			{/if}
			{#if data.syncing_count > 0}
				<button class="summary-chip chip-syncing {filter === 'syncing' ? 'active' : ''}" onclick={() => (filter = 'syncing')}>
					Syncing <span class="chip-count">{data.syncing_count}</span>
				</button>
			{/if}
			{#if data.unprocessed_count > 0}
				<button class="summary-chip chip-unprocessed {filter === 'unprocessed' ? 'active' : ''}" onclick={() => (filter = 'unprocessed')}>
					Unprocessed <span class="chip-count">{data.unprocessed_count}</span>
				</button>
			{/if}
		</div>

		{#if visibleRows.length === 0}
			<p class="empty">
				{filter === 'all' ? 'No records to display.' : 'No records match this filter.'}
			</p>
		{:else}
			<SyncTable rows={visibleRows} {syncState} {triggerSync} {displayName} {healthLabel} {pgStatusLabel} {syncBtnLabel} {syncBtnTitle} />
		{/if}
	{/if}
</div>

<!-- Shared table snippet used for both the main listing and the lookup result -->
{#snippet SyncTable(
	rows: SyncStatusRow[],
	syncState: Record<string, string>,
	triggerSync: (k: string) => void,
	displayName: (r: SyncStatusRow) => string,
	healthLabel: Record<string, string>,
	pgStatusLabel: Record<string, string>,
	syncBtnLabel: (k: string) => string,
	syncBtnTitle: (k: string) => string
)}
	<div class="table-wrap">
		<table>
			<thead>
				<tr>
					<th>Health</th>
					<th>Participant</th>
					<th>Part key</th>
					<th>Type</th>
					<th>Informix</th>
					<th>PostgreSQL</th>
					<th>Decision</th>
					<th>Sync queue</th>
					<th></th>
				</tr>
			</thead>
			<tbody>
				{#each rows as row}
					{@const ss = syncState[row.part_key]}
					<tr class="row-{row.health}">
						<td>
							<span class="badge badge-{row.health}">{healthLabel[row.health] ?? row.health}</span>
							{#if row.health_reason}
								<div class="health-reason">{row.health_reason}</div>
							{/if}
							{#if ss?.startsWith('error:')}
								<div class="health-reason">{ss.slice(6)}</div>
							{/if}
						</td>
						<td>
							<a class="name-link" href="/reviews/{row.part_key}">{displayName(row)}</a>
							<div class="sub">{row.part_no}</div>
						</td>
						<td class="mono">{row.part_key}</td>
						<td class="type-cell">{row.review_type}</td>
						<td>
							{#if row.ifx_status === null}
								<span class="text-muted">Not found</span>
							{:else if row.ifx_status === 'P'}
								<span class="badge badge-active">Open</span>
							{:else}
								<span class="badge badge-ok">Closed ({row.ifx_status})</span>
							{/if}
						</td>
						<td>
							{#if row.pg_status === null}
								<span class="text-muted">—</span>
							{:else}
								<span class="badge badge-pg">{pgStatusLabel[row.pg_status] ?? row.pg_status}</span>
							{/if}
						</td>
						<td>
							{row.pg_decision ?? '—'}
						</td>
						<td>
							{#if row.sync_failed > 0}
								<span class="badge badge-error">{row.sync_failed} failed</span>
								{#each row.sync_errors as err}
									<div class="sync-error">{err}</div>
								{/each}
							{:else if row.sync_pending > 0}
								<span class="badge badge-syncing">{row.sync_pending} pending</span>
							{:else}
								<span class="text-muted">—</span>
							{/if}
						</td>
						<td class="actions-cell">
							{#if row.sync_pending > 0 || row.sync_failed > 0}
								<button
									class="sync-btn {ss === 'syncing' ? 'syncing' : ''} {ss?.startsWith('error:') ? 'errored' : ''} {ss === 'done' ? 'done' : ''}"
									onclick={() => triggerSync(row.part_key)}
									disabled={ss === 'syncing'}
									title={syncBtnTitle(row.part_key)}
								>
									{syncBtnLabel(row.part_key)}
								</button>
							{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{/snippet}

<style>
	.page-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1rem;
		margin-bottom: 1.25rem;
	}

	/* Lookup bar */
	.lookup-bar {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	.lookup-input {
		flex: 1;
		padding: 0.45rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-size: 0.875rem;
		background: var(--surface);
		color: var(--text);
		min-width: 0;
	}
	.lookup-input:focus {
		outline: none;
		border-color: var(--gold);
		box-shadow: 0 0 0 3px rgba(181, 152, 90, 0.15);
	}

	/* Lookup result */
	.lookup-result {
		background: var(--surface);
		border: 2px solid var(--gold);
		border-radius: var(--radius);
		margin-bottom: 1.5rem;
		overflow: hidden;
	}

	.lookup-result-header {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.6rem 0.9rem;
		background: rgba(181, 152, 90, 0.08);
		border-bottom: 1px solid var(--border);
		font-size: 0.875rem;
	}

	.clear-btn {
		margin-left: auto;
		background: none;
		border: none;
		cursor: pointer;
		color: var(--text-muted);
		font-size: 0.9rem;
		padding: 0 0.25rem;
		line-height: 1;
	}
	.clear-btn:hover { color: var(--text); }

	.loading { color: var(--text-muted); padding: 2rem 0; }

	.alert-error {
		background: #fef2f2;
		border: 1px solid #fca5a5;
		color: #991b1b;
		padding: 0.75rem 1rem;
		border-radius: var(--radius);
		margin-bottom: 1rem;
		font-size: 0.875rem;
	}

	/* Summary filter bar */
	.summary-bar {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		margin-bottom: 1.25rem;
	}

	.summary-chip {
		display: inline-flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.3rem 0.75rem;
		border-radius: 999px;
		font-size: 0.82rem;
		font-weight: 600;
		border: 1px solid var(--border);
		background: var(--surface);
		color: var(--text);
		cursor: pointer;
		transition: background 0.12s, border-color 0.12s;
	}
	.summary-chip:hover,
	.summary-chip.active { background: var(--navy); color: #fff; border-color: var(--navy); }
	.chip-error { border-color: #fca5a5; color: #991b1b; }
	.chip-error.active { background: #991b1b; border-color: #991b1b; color: #fff; }
	.chip-warning { border-color: #fcd34d; color: #92400e; }
	.chip-warning.active { background: #92400e; border-color: #92400e; color: #fff; }
	.chip-syncing { border-color: #93c5fd; color: #1e40af; }
	.chip-syncing.active { background: #1e40af; border-color: #1e40af; color: #fff; }
	.chip-unprocessed { border-color: #d1d5db; color: #374151; }
	.chip-unprocessed.active { background: #374151; border-color: #374151; color: #fff; }

	.chip-count {
		background: rgba(0, 0, 0, 0.12);
		border-radius: 999px;
		padding: 0 0.4rem;
		font-size: 0.78rem;
	}

	.empty { color: var(--text-muted); padding: 1.5rem 0; }

	/* Table */
	.table-wrap {
		overflow-x: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius);
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.875rem;
	}

	thead th {
		background: var(--surface);
		padding: 0.6rem 0.9rem;
		text-align: left;
		font-size: 0.75rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-muted);
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
	}

	tbody tr { border-bottom: 1px solid var(--border); }
	tbody tr:last-child { border-bottom: none; }
	tbody tr:hover { background: rgba(0, 0, 0, 0.02); }
	tbody tr.row-error { background: #fef9f9; }
	tbody tr.row-warning { background: #fffdf0; }

	td {
		padding: 0.65rem 0.9rem;
		vertical-align: top;
	}

	.mono {
		font-family: var(--font-mono, monospace);
		font-size: 0.8rem;
		color: var(--text-muted);
	}

	.type-cell { text-transform: capitalize; }

	.name-link {
		color: var(--text);
		text-decoration: none;
		font-weight: 500;
	}
	.name-link:hover { text-decoration: underline; color: var(--navy); }

	.sub { font-size: 0.78rem; color: var(--text-muted); }
	.text-muted { color: var(--text-muted); }

	.health-reason {
		font-size: 0.75rem;
		color: #991b1b;
		margin-top: 0.25rem;
		max-width: 180px;
	}

	.sync-error {
		font-size: 0.72rem;
		color: #991b1b;
		margin-top: 0.2rem;
		max-width: 200px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.actions-cell { white-space: nowrap; }

	/* Sync button */
	.sync-btn {
		padding: 0.25rem 0.6rem;
		font-size: 0.78rem;
		font-weight: 600;
		border-radius: var(--radius-sm);
		border: 1px solid var(--navy);
		background: transparent;
		color: var(--navy);
		cursor: pointer;
		transition: background 0.12s, color 0.12s;
	}
	.sync-btn:hover:not(:disabled) { background: var(--navy); color: #fff; }
	.sync-btn:disabled { opacity: 0.5; cursor: default; }
	.sync-btn.syncing { border-color: #93c5fd; color: #1e40af; }
	.sync-btn.errored { border-color: #fca5a5; color: #991b1b; }
	.sync-btn.done { border-color: #86efac; color: #166534; }

	/* Health badges */
	.badge {
		display: inline-block;
		padding: 0.15rem 0.55rem;
		border-radius: 999px;
		font-size: 0.75rem;
		font-weight: 600;
		white-space: nowrap;
	}
	.badge-ok { background: #dcfce7; color: #166534; }
	.badge-active { background: #dbeafe; color: #1e40af; }
	.badge-syncing { background: #e0f2fe; color: #0369a1; }
	.badge-warning { background: #fef9c3; color: #92400e; }
	.badge-error { background: #fee2e2; color: #991b1b; }
	.badge-unprocessed { background: #f3f4f6; color: #374151; }
	.badge-pg { background: #f3e8ff; color: #6b21a8; }
</style>
