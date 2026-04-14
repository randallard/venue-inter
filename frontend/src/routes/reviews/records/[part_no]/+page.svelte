<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getParticipantCheck } from '$lib/api';
	import type { ParticipantCheck } from '$lib/types';

	const part_no = $derived($page.params.part_no ?? '');

	let data = $state<ParticipantCheck | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			data = await getParticipantCheck(part_no);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	const PM_STATUS: Record<number, string> = {
		1: 'Summoned',
		2: 'Qualified',
		5: 'Perm Excuse',
		6: 'Disqualified',
		7: 'Temp Excuse',
	};

	const IFX_STATUS: Record<string, string> = {
		P: 'Pending Admin',
		S: 'Sent to CEO',
		C: 'Completed',
	};

	const PG_STATUS: Record<string, string> = {
		pending_admin: 'Pending Admin',
		pending_ceo:   'CEO Queue',
		completed:     'Completed',
		sent_back:     'Sent Back',
	};

	const SYNC_STATUS: Record<string, string> = {
		pending:   'Pending',
		completed: 'Done',
		failed:    'Failed',
	};


	const ACTION_LABEL: Record<string, string> = {
		submitted:         'Submitted',
		sent_to_ceo:       'Sent to CEO',
		sent_back:         'Sent Back to Admin',
		recalled:          'Recalled to Admin',
		requalify:         'Re-qualified',
		disqualify:        'Disqualified',
		permanent_excuse:  'Permanent Excuse',
		temporary_excuse:  'Temporary Excuse',
		completed:         'Completed',
	};

	const ACTION_CLASS: Record<string, string> = {
		submitted:        'a-submitted',
		sent_to_ceo:      'a-sent',
		sent_back:        'a-back',
		recalled:         'a-back',
		requalify:        'a-positive',
		disqualify:       'a-negative',
		permanent_excuse: 'a-excuse',
		temporary_excuse: 'a-excuse',
		completed:        'a-positive',
	};

	function fmt(iso: string | null) {
		if (!iso) return '—';
		return new Date(iso).toLocaleString();
	}
</script>

<div class="container">
	<a class="back-link" href="/reviews">Reviews</a>

	<div class="page-header">
		<h1>Participant Check</h1>
		{#if data}
			<p>#{part_no}{data.fname || data.lname ? ` — ${[data.fname, data.lname].filter(Boolean).join(' ')}` : ''}</p>
		{:else}
			<p>#{part_no}</p>
		{/if}
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
		<button class="btn btn-secondary btn-sm" onclick={load}>Retry</button>
	{:else if data}

		<!-- ── Informix ───────────────────────────────────── -->
		<section class="check-section">
			<h2 class="section-title">
				<span class="section-badge ifx">Informix</span>
				Pool Members
			</h2>
			{#if data.pool_members.length === 0}
				<p class="text-muted empty-msg">No pool_member rows found.</p>
			{:else}
				<div class="table-wrap">
					<table class="check-table">
						<thead>
							<tr>
								<th>Pool</th>
								<th>Show</th>
								<th>Status</th>
								<th>Scan Code</th>
							</tr>
						</thead>
						<tbody>
							{#each data.pool_members as pm}
								<tr>
									<td>{pm.pool_no}</td>
									<td>{pm.show_no ?? '—'}</td>
									<td>
										<span class="status-pill pm-{pm.status}">
											{PM_STATUS[pm.status] ?? pm.status}
										</span>
									</td>
									<td class="mono">{pm.scan_code ?? '—'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<section class="check-section">
			<h2 class="section-title">
				<span class="section-badge ifx">Informix</span>
				Review Records
			</h2>
			{#if data.review_records.length === 0}
				<p class="text-muted empty-msg">No review_record rows found.</p>
			{:else}
				<div class="table-wrap">
					<table class="check-table">
						<thead>
							<tr>
								<th>Record #</th>
								<th>Pool</th>
								<th>Type</th>
								<th>Status</th>
								<th>Submitted</th>
							</tr>
						</thead>
						<tbody>
							{#each data.review_records as rr}
								<tr>
									<td class="mono">{rr.rr_id}</td>
									<td>{rr.pool_no}</td>
									<td class="capitalize">{rr.review_type}</td>
									<td>
										<span class="status-pill ifx-{rr.ifx_status}">
											{IFX_STATUS[rr.ifx_status] ?? rr.ifx_status}
										</span>
									</td>
									<td>{rr.submitted_date ?? '—'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<!-- ── PostgreSQL ─────────────────────────────────── -->
		<section class="check-section">
			<h2 class="section-title">
				<span class="section-badge pg">PostgreSQL</span>
				Status Reviews
			</h2>
			{#if data.status_reviews.length === 0}
				<p class="text-muted empty-msg">No status_reviews rows found.</p>
			{:else}
				<div class="table-wrap">
					<table class="check-table">
						<thead>
							<tr>
								<th>Part Key</th>
								<th>Type</th>
								<th>PG Status</th>
								<th>Decision</th>
								<th>Sent to CEO</th>
								<th>Decided</th>
								<th>Updated</th>
							</tr>
						</thead>
						<tbody>
							{#each data.status_reviews as sr}
								<tr>
									<td class="mono">{sr.part_key}</td>
									<td class="capitalize">{sr.review_type}</td>
									<td>
										<span class="status-pill pg-{sr.pg_status}">
											{PG_STATUS[sr.pg_status] ?? sr.pg_status}
										</span>
									</td>
									<td class="capitalize">{sr.decision?.replace('_', ' ') ?? '—'}</td>
									<td>{fmt(sr.sent_to_ceo_at)}</td>
									<td>{fmt(sr.decided_at)}</td>
									<td>{fmt(sr.updated_at)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<section class="check-section">
			<h2 class="section-title">
				<span class="section-badge pg">PostgreSQL</span>
				Sync Queue
			</h2>
			{#if data.sync_queue.length === 0}
				<p class="text-muted empty-msg">No sync queue entries for this participant.</p>
			{:else}
				<div class="table-wrap">
					<table class="check-table">
						<thead>
							<tr>
								<th>Operation</th>
								<th>Status</th>
								<th>Attempts</th>
								<th>Error</th>
								<th>Created</th>
								<th>Completed</th>
							</tr>
						</thead>
						<tbody>
							{#each data.sync_queue as sq}
								<tr>
									<td class="mono">{sq.operation}</td>
									<td>
										<span class="status-pill sq-{sq.status}">
											{SYNC_STATUS[sq.status] ?? sq.status}
										</span>
									</td>
									<td>{sq.attempts}</td>
									<td class="error-cell">{sq.last_error ?? '—'}</td>
									<td>{fmt(sq.created_at)}</td>
									<td>{fmt(sq.completed_at)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<!-- ── Documents ──────────────────────────────────── -->
		<section class="check-section">
			<h2 class="section-title">
				<span class="section-badge doc">Documents</span>
				Informix Documents
			</h2>
			{#if data.documents.length === 0}
				<p class="text-muted empty-msg">No part_image rows found in Informix for this participant.</p>
			{:else}
				<div class="table-wrap">
					<table class="check-table">
						<thead>
							<tr>
								<th>File</th>
								<th>WebDAV Path</th>
								<th></th>
							</tr>
						</thead>
						<tbody>
							{#each data.documents as doc}
								<tr>
									<td>{doc.file_name}</td>
									<td class="mono path">{doc.webdav_path}</td>
									<td>
										<a
											class="btn btn-secondary btn-sm"
											href="/api/documents/view?path={encodeURIComponent(doc.webdav_path)}"
											target="_blank"
											rel="noopener"
										>View</a>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		<!-- ── Audit Trail ────────────────────────────────── -->
		<section class="check-section">
			<h2 class="section-title">Audit Trail</h2>
			{#if data.history.length === 0}
				<p class="text-muted empty-msg">No review history on record.</p>
			{:else}
				<p class="count-label">{data.history.length} {data.history.length === 1 ? 'entry' : 'entries'}</p>
				<div class="timeline">
					{#each data.history as entry}
						<div class="timeline-item">
							<div class="timeline-marker {ACTION_CLASS[entry.action] ?? ''}"></div>
							<div class="card timeline-card">
								<div class="entry-header">
									<span class="action-badge {ACTION_CLASS[entry.action] ?? ''}">
										{ACTION_LABEL[entry.action] ?? entry.action}
									</span>
									<span class="entry-type">{entry.review_type}</span>
									<time class="entry-time">{fmt(entry.acted_at)}</time>
								</div>
								{#if entry.actor_email}
									<p class="entry-actor">{entry.actor_email}</p>
								{/if}
								{#if entry.notes}
									<p class="entry-notes">{entry.notes}</p>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>

	{/if}
</div>

<style>
	.check-section {
		margin-bottom: 2rem;
	}

	.section-title {
		font-size: 0.95rem;
		font-weight: 600;
		margin-bottom: 0.65rem;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.section-badge {
		font-size: 0.68rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		padding: 0.15rem 0.45rem;
		border-radius: 3px;
	}
	.section-badge.ifx { background: #dbeafe; color: #1d4ed8; }
	.section-badge.pg  { background: #dcfce7; color: #15803d; }
	.section-badge.doc { background: #ede9fe; color: #6d28d9; }

	.table-wrap { overflow-x: auto; }

	.check-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.875rem;
	}

	.check-table th {
		text-align: left;
		padding: 0.4rem 0.75rem;
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-muted);
		border-bottom: 1px solid var(--border);
		white-space: nowrap;
	}

	.check-table td {
		padding: 0.5rem 0.75rem;
		border-bottom: 1px solid var(--border);
		vertical-align: top;
	}

	.check-table tr:last-child td { border-bottom: none; }
	.check-table tbody tr:hover { background: var(--surface-alt, #f9fafb); }

	.mono  { font-family: monospace; font-size: 0.82rem; }
	.path  { word-break: break-all; max-width: 260px; }
	.capitalize { text-transform: capitalize; }

	.empty-msg { font-size: 0.875rem; padding: 0.5rem 0; }

	.error-cell {
		font-size: 0.8rem;
		color: var(--red, #dc2626);
		max-width: 200px;
		word-break: break-word;
	}

	/* Status pills */
	.status-pill {
		display: inline-block;
		padding: 0.15rem 0.5rem;
		border-radius: 3px;
		font-size: 0.78rem;
		font-weight: 600;
		white-space: nowrap;
		background: var(--surface-alt, #f3f4f6);
		color: var(--text-muted);
	}

	/* pool_member status */
	.pm-1 { background: #dbeafe; color: #1d4ed8; }   /* summoned */
	.pm-2 { background: #dcfce7; color: #15803d; }   /* qualified */
	.pm-5 { background: #ede9fe; color: #6d28d9; }   /* perm excuse */
	.pm-6 { background: #fee2e2; color: #b91c1c; }   /* disqualified */
	.pm-7 { background: #fef9c3; color: #92400e; }   /* temp excuse */

	/* Informix review_record status */
	.ifx-P { background: #fef9c3; color: #92400e; }  /* pending */
	.ifx-S { background: #dbeafe; color: #1d4ed8; }  /* sent to CEO */
	.ifx-C { background: #dcfce7; color: #15803d; }  /* completed */

	/* PG status */
	.pg-pending_admin { background: #fef9c3; color: #92400e; }
	.pg-pending_ceo   { background: #dbeafe; color: #1d4ed8; }
	.pg-completed     { background: #dcfce7; color: #15803d; }
	.pg-sent_back     { background: #f3f4f6; color: #374151; }

	/* Sync queue status */
	.sq-pending   { background: #fef9c3; color: #92400e; }
	.sq-completed { background: #dcfce7; color: #15803d; }
	.sq-failed    { background: #fee2e2; color: #b91c1c; }


	/* Audit trail */
	.count-label {
		font-size: 0.88rem;
		color: var(--text-muted);
		margin-bottom: 1rem;
	}

	.timeline {
		display: flex;
		flex-direction: column;
		gap: 0;
		position: relative;
	}

	.timeline::before {
		content: '';
		position: absolute;
		left: 0.65rem;
		top: 0.75rem;
		bottom: 0.75rem;
		width: 2px;
		background: var(--border);
	}

	.timeline-item {
		display: flex;
		gap: 1rem;
		align-items: flex-start;
		padding-bottom: 1rem;
	}

	.timeline-marker {
		flex-shrink: 0;
		width: 1.35rem;
		height: 1.35rem;
		border-radius: 50%;
		margin-top: 0.6rem;
		background: var(--border);
		border: 2px solid var(--surface);
		z-index: 1;
	}

	.timeline-marker.a-positive   { background: var(--green, #16a34a); }
	.timeline-marker.a-negative   { background: var(--red, #dc2626); }
	.timeline-marker.a-sent       { background: var(--gold, #b5985a); }
	.timeline-marker.a-back       { background: var(--text-muted, #6b7280); }
	.timeline-marker.a-excuse     { background: #7c3aed; }
	.timeline-marker.a-submitted  { background: #2563eb; }

	.timeline-card { flex: 1; padding: 0.75rem 1rem; }

	.entry-header {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		flex-wrap: wrap;
	}

	.action-badge {
		font-size: 0.82rem;
		font-weight: 600;
		padding: 0.15rem 0.5rem;
		border-radius: 3px;
		background: var(--surface-alt, #f3f4f6);
		color: var(--text);
	}

	.action-badge.a-positive  { background: #dcfce7; color: #15803d; }
	.action-badge.a-negative  { background: #fee2e2; color: #b91c1c; }
	.action-badge.a-sent      { background: #fef9c3; color: #92400e; }
	.action-badge.a-back      { background: #f3f4f6; color: #374151; }
	.action-badge.a-excuse    { background: #ede9fe; color: #6d28d9; }
	.action-badge.a-submitted { background: #dbeafe; color: #1d4ed8; }

	.entry-type  { font-size: 0.82rem; color: var(--text-muted); text-transform: capitalize; }
	.entry-time  { font-size: 0.82rem; color: var(--text-muted); margin-left: auto; }
	.entry-actor { margin: 0.35rem 0 0; font-size: 0.85rem; color: var(--text-muted); }
	.entry-notes { margin: 0.35rem 0 0; font-size: 0.9rem; color: var(--text); font-style: italic; }
</style>
