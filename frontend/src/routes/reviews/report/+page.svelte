<script lang="ts">
	import { onMount } from 'svelte';
	import { getReviewReport } from '$lib/api';
	import type { ReviewReportRow } from '$lib/types';

	let rows = $state<ReviewReportRow[]>([]);
	let count = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function load() {
		loading = true;
		error = null;
		try {
			const res = await getReviewReport();
			rows = res.rows;
			count = res.count;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	const DECISION_LABEL: Record<string, string> = {
		requalify:         'Re-qualified',
		disqualify:        'Disqualified',
		permanent_excuse:  'Perm Excuse',
		temporary_excuse:  'Temp Excuse',
	};

	const DECISION_CLASS: Record<string, string> = {
		requalify:         'dec-positive',
		disqualify:        'dec-negative',
		permanent_excuse:  'dec-excuse',
		temporary_excuse:  'dec-excuse',
	};

	const SYNC_LABEL: Record<string, string> = {
		done:    'Synced',
		pending: 'Pending',
		failed:  'Failed',
		no_ops:  'No ops',
	};

	const SYNC_CLASS: Record<string, string> = {
		done:    'sync-done',
		pending: 'sync-pending',
		failed:  'sync-failed',
		no_ops:  'sync-noops',
	};

	function fmt(iso: string | null) {
		if (!iso) return '—';
		return new Date(iso).toLocaleString();
	}

	function displayName(row: ReviewReportRow) {
		const name = [row.fname, row.lname].filter(Boolean).join(' ');
		return name || `#${row.part_no}`;
	}
</script>

<div class="container">
	<a class="back-link" href="/reviews">Reviews</a>

	<div class="page-header">
		<h1>Decisions Report</h1>
		<p>Excuse and disqualification cases decided in the last 90 days</p>
	</div>

	<div class="toolbar">
		<button class="btn btn-secondary btn-sm" onclick={load} disabled={loading}>
			{loading ? 'Loading…' : 'Refresh'}
		</button>
		{#if !loading && !error}
			<span class="count-label">{count} {count === 1 ? 'record' : 'records'}</span>
		{/if}
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
		<button class="btn btn-secondary btn-sm" onclick={load}>Retry</button>
	{:else if rows.length === 0}
		<div class="card empty-card">
			<p class="text-muted">No decided cases in the last 90 days.</p>
		</div>
	{:else}
		<div class="table-wrap">
			<table class="report-table">
				<thead>
					<tr>
						<th>Participant</th>
						<th>Pool</th>
						<th>Type</th>
						<th>Decision</th>
						<th>Decided</th>
						<th>Sync</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each rows as row}
						<tr>
							<td>
								<span class="part-name">{displayName(row)}</span>
								<span class="part-no">#{row.part_no}</span>
							</td>
							<td>{row.pool_no}</td>
							<td class="capitalize">{row.review_type}</td>
							<td>
								{#if row.decision}
									<span class="pill {DECISION_CLASS[row.decision] ?? ''}">
										{DECISION_LABEL[row.decision] ?? row.decision}
									</span>
								{:else}
									<span class="pill dec-back">Sent Back</span>
								{/if}
							</td>
							<td class="date-cell">{fmt(row.decided_at)}</td>
							<td>
								<span class="pill {SYNC_CLASS[row.sync_status] ?? ''}">
									{SYNC_LABEL[row.sync_status] ?? row.sync_status}
								</span>
							</td>
							<td>
								<a class="record-link" href="/reviews/records/{row.part_no}">
									View record
								</a>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>

<style>
	.toolbar {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin-bottom: 1rem;
	}

	.count-label {
		font-size: 0.85rem;
		color: var(--text-muted);
	}

	.table-wrap { overflow-x: auto; }

	.report-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.875rem;
	}

	.report-table th {
		text-align: left;
		padding: 0.4rem 0.85rem;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-muted);
		border-bottom: 2px solid var(--border);
		white-space: nowrap;
	}

	.report-table td {
		padding: 0.6rem 0.85rem;
		border-bottom: 1px solid var(--border);
		vertical-align: middle;
	}

	.report-table tr:last-child td { border-bottom: none; }
	.report-table tbody tr:hover   { background: var(--surface-alt, #f9fafb); }

	.part-name { font-weight: 500; display: block; }
	.part-no   { font-size: 0.78rem; color: var(--text-muted); }

	.capitalize { text-transform: capitalize; }

	.date-cell { white-space: nowrap; color: var(--text-muted); font-size: 0.85rem; }

	.pill {
		display: inline-block;
		padding: 0.18rem 0.55rem;
		border-radius: 3px;
		font-size: 0.78rem;
		font-weight: 600;
		white-space: nowrap;
		background: var(--surface-alt, #f3f4f6);
		color: var(--text-muted);
	}

	/* Decision pills */
	.dec-positive { background: #dcfce7; color: #15803d; }
	.dec-negative { background: #fee2e2; color: #b91c1c; }
	.dec-excuse   { background: #ede9fe; color: #6d28d9; }
	.dec-back     { background: #f3f4f6; color: #374151; }

	/* Sync pills */
	.sync-done    { background: #dcfce7; color: #15803d; }
	.sync-pending { background: #fef9c3; color: #92400e; }
	.sync-failed  { background: #fee2e2; color: #b91c1c; }
	.sync-noops   { background: #f3f4f6; color: #6b7280; }

	.record-link {
		font-size: 0.82rem;
		color: var(--navy, #1e3a5f);
		text-decoration: none;
		font-weight: 500;
	}
	.record-link:hover { text-decoration: underline; }

	.empty-card { padding: 2rem; text-align: center; }
</style>
