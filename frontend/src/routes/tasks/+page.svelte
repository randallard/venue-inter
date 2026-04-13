<script lang="ts">
	import { onMount } from 'svelte';
	import { getTasks, getTickets } from '$lib/api';
	import type { TaskRow, TicketRow } from '$lib/types';

	let tasks = $state<TaskRow[]>([]);
	let tickets = $state<TicketRow[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			const [tasksRes, ticketsRes] = await Promise.all([getTasks(), getTickets()]);
			tasks = tasksRes.tasks;
			tickets = ticketsRes;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	onMount(load);

	function statusClass(status: string): string {
		if (status === 'failed') return 'badge-red';
		if (status === 'completed') return 'badge-green';
		if (status === 'running') return 'badge-blue';
		return 'badge-grey';
	}

	function fmtDate(iso: string): string {
		return new Date(iso).toLocaleString();
	}
</script>

<div class="container">
	<a class="back-link" href="/">Dashboard</a>

	<div class="page-header">
		<h1>Tasks</h1>
		<p>Background operations and failure tickets for your account</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else}
		<section>
			<h2>Recent Tasks</h2>
			{#if tasks.length === 0}
				<p class="text-muted">No tasks yet.</p>
			{:else}
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th>Type</th>
								<th>Description</th>
								<th>Status</th>
								<th>Result</th>
								<th>Created</th>
							</tr>
						</thead>
						<tbody>
							{#each tasks as task (task.id)}
								<tr class:row-failed={task.status === 'failed'}>
									<td class="type-cell">{task.task_type}</td>
									<td>{task.description}</td>
									<td><span class="badge {statusClass(task.status)}">{task.status}</span></td>
									<td class="detail-cell">
										{#if task.status === 'failed' && task.error_detail}
											<span class="error-text" title={task.error_detail}>
												{task.error_detail.length > 80
													? task.error_detail.slice(0, 80) + '…'
													: task.error_detail}
											</span>
										{:else if task.result_summary}
											{task.result_summary}
										{:else}
											—
										{/if}
									</td>
									<td class="date-cell">{fmtDate(task.created_at)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		</section>

		{#if tickets.length > 0}
			<section>
				<h2>Open Tickets</h2>
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th>Description</th>
								<th>Status</th>
								<th>Created</th>
							</tr>
						</thead>
						<tbody>
							{#each tickets as ticket (ticket.id)}
								<tr>
									<td>{ticket.description}</td>
									<td><span class="badge badge-red">{ticket.status}</span></td>
									<td class="date-cell">{fmtDate(ticket.created_at)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>
		{/if}

		<div class="refresh-row">
			<button class="btn btn-secondary" onclick={load} disabled={loading}>Refresh</button>
		</div>
	{/if}
</div>

<style>
	section {
		margin-bottom: 2rem;
	}

	h2 {
		font-size: 1rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-muted);
		margin-bottom: 0.75rem;
	}

	.type-cell {
		font-family: monospace;
		font-size: 0.82rem;
		white-space: nowrap;
	}

	.date-cell {
		white-space: nowrap;
		font-size: 0.82rem;
		color: var(--text-muted);
	}

	.detail-cell {
		font-size: 0.85rem;
		max-width: 340px;
	}

	.error-text {
		color: var(--red);
	}

	.row-failed td {
		background: color-mix(in srgb, var(--red) 6%, transparent);
	}

	.badge {
		display: inline-block;
		padding: 0.15rem 0.55rem;
		border-radius: 999px;
		font-size: 0.75rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.badge-red   { background: color-mix(in srgb, var(--red)   15%, transparent); color: var(--red); }
	.badge-green { background: color-mix(in srgb, var(--green) 15%, transparent); color: var(--green); }
	.badge-blue  { background: color-mix(in srgb, #3b82f6      15%, transparent); color: #1d4ed8; }
	.badge-grey  { background: var(--border); color: var(--text-muted); }

	.refresh-row {
		display: flex;
		justify-content: flex-end;
	}
</style>
