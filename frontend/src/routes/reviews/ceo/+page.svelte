<script lang="ts">
	import { onMount } from 'svelte';
	import { getCeoQueue } from '$lib/api';
	import type { CeoReviewRow } from '$lib/types';

	let rows = $state<CeoReviewRow[]>([]);
	let maintenance = $state(false);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			const data = await getCeoQueue();
			rows = data.rows;
			maintenance = data.maintenance;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load queue';
		} finally {
			loading = false;
		}
	}

	function formatDate(iso: string | null): string {
		if (!iso) return '—';
		return new Date(iso).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	onMount(load);
</script>

<div class="container">
	<div class="page-header">
		<h1>Review Queue</h1>
		<p>Cases prepared for your decision</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if maintenance}
		<div class="card maintenance">
			<div class="maint-icon">⚙</div>
			<h2>Queue in Maintenance Mode</h2>
			<p>The review queue is temporarily unavailable. Please check back shortly.</p>
		</div>
	{:else if rows.length === 0}
		<div class="card empty">
			<span class="ok-icon">✓</span>
			<p>No cases pending your review.</p>
		</div>
	{:else}
		<div class="count-label">{rows.length} case{rows.length !== 1 ? 's' : ''} awaiting decision</div>
		<div class="table-wrap">
			<table>
				<thead>
					<tr>
						<th>Participant</th>
						<th>Type</th>
						<th>Pool</th>
						<th>Sent</th>
						<th>Notes</th>
						<th></th>
					</tr>
				</thead>
				<tbody>
					{#each rows as row}
						<tr>
							<td>
								<div class="name">{row.lname ?? '—'}, {row.fname ?? '—'}</div>
								<div class="sub">Part #{row.part_no}</div>
							</td>
							<td>
								<span class="type-badge type-{row.review_type}">
									{row.review_type}
								</span>
							</td>
							<td>#{row.pool_no}</td>
							<td class="date-cell">{formatDate(row.sent_to_ceo_at)}</td>
							<td class="notes-cell">{row.admin_notes ?? '—'}</td>
							<td>
								<a class="btn btn-primary btn-sm" href="/reviews/ceo/{row.part_key}">
									Decide
								</a>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	<div class="refresh-row">
		<button class="btn btn-secondary" onclick={load} disabled={loading}>Refresh</button>
	</div>
</div>

<style>
	.name { font-weight: 600; }
	.sub { font-size: 0.8rem; color: var(--text-muted); }
	.date-cell { white-space: nowrap; }
	.notes-cell { max-width: 220px; color: var(--text-muted); font-size: 0.88rem; }
	.btn-sm { padding: 0.3rem 0.7rem; font-size: 0.82rem; text-decoration: none; }

	.type-badge {
		display: inline-block;
		padding: 0.15rem 0.55rem;
		border-radius: 3px;
		font-size: 0.78rem;
		font-weight: 700;
		text-transform: capitalize;
	}
	.type-excuse { background: #fef9c3; color: #854d0e; }
	.type-disqualify { background: #fef2f2; color: #991b1b; }

	.card.empty, .card.maintenance {
		text-align: center;
		padding: 3rem;
		color: var(--text-muted);
	}
	.ok-icon { font-size: 2rem; color: var(--green); display: block; margin-bottom: 0.5rem; }
	.maint-icon { font-size: 2.5rem; margin-bottom: 0.75rem; }
	.maintenance h2 { margin-bottom: 0.5rem; }

	.refresh-row { display: flex; justify-content: flex-end; margin-top: 1rem; }
</style>
