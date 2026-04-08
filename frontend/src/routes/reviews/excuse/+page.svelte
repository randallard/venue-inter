<script lang="ts">
	import { onMount } from 'svelte';
	import { getAdminReviewQueue } from '$lib/api';
	import type { AdminReviewRow } from '$lib/types';

	let rows = $state<AdminReviewRow[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			const data = await getAdminReviewQueue('excuse');
			rows = data.rows;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	onMount(load);
</script>

<div class="container">
	<a class="back-link" href="/reviews">Reviews</a>

	<div class="page-header">
		<h1>Excuse Requests</h1>
		<p>Pending admin review — verify and send to CEO</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if rows.length === 0}
		<div class="card empty">
			<span class="ok-icon">✓</span>
			<p>No excuse requests pending.</p>
		</div>
	{:else}
		<div class="count-label">{rows.length} pending request{rows.length !== 1 ? 's' : ''}</div>
		<div class="table-wrap">
			<table>
				<thead>
					<tr>
						<th>Participant</th>
						<th>Pool</th>
						<th>Submitted</th>
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
							<td>#{row.pool_no}</td>
							<td>{row.submitted_date ?? '—'}</td>
							<td class="notes-cell">{row.admin_notes ?? '—'}</td>
							<td>
								<a class="btn btn-primary btn-sm" href="/reviews/excuse/{row.part_key}">
									Review
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
	.name { font-weight: 600; }
	.sub { font-size: 0.8rem; color: var(--text-muted); }
	.notes-cell { max-width: 260px; color: var(--text-muted); font-size: 0.88rem; }
	.btn-sm { padding: 0.3rem 0.7rem; font-size: 0.82rem; text-decoration: none; }
	.card.empty { text-align: center; padding: 2.5rem; color: var(--text-muted); }
	.ok-icon { font-size: 2rem; color: var(--green); display: block; margin-bottom: 0.5rem; }
</style>
