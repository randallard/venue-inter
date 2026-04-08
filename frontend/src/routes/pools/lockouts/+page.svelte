<script lang="ts">
	import { onMount } from 'svelte';
	import { getPortalLockouts, unlockParticipant } from '$lib/api';
	import type { PortalLockoutRow } from '$lib/types';

	let rows = $state<PortalLockoutRow[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let search = $state('');

	let filtered = $derived(
		search.trim()
			? rows.filter((r) => {
					const q = search.toLowerCase();
					return (
						String(r.part_no).includes(q) ||
						(r.fname ?? '').toLowerCase().includes(q) ||
						(r.lname ?? '').toLowerCase().includes(q)
					);
			  })
			: rows
	);

	type RowState = 'idle' | 'pending' | 'ok' | 'err';
	let rowStates = $state<Record<number, RowState>>({});
	let rowMessages = $state<Record<number, string>>({});

	let confirmRow = $state<PortalLockoutRow | null>(null);

	function openConfirm(row: PortalLockoutRow) {
		confirmRow = row;
	}

	function closeConfirm() {
		confirmRow = null;
	}

	async function doUnlock() {
		if (!confirmRow) return;
		const part_no = confirmRow.part_no;
		const name = `${confirmRow.fname ?? ''} ${confirmRow.lname ?? ''}`.trim();
		confirmRow = null;
		rowStates[part_no] = 'pending';
		try {
			const res = await unlockParticipant(part_no);
			rowStates[part_no] = res.ok ? 'ok' : 'err';
			rowMessages[part_no] = res.message;
			if (res.ok) {
				setTimeout(() => {
					rows = rows.filter((r) => r.part_no !== part_no);
				}, 1200);
			}
		} catch (e) {
			rowStates[part_no] = 'err';
			rowMessages[part_no] = e instanceof Error ? e.message : 'Unlock failed';
		}
	}

	async function load() {
		loading = true;
		error = null;
		try {
			const data = await getPortalLockouts();
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
	<a class="back-link" href="/">Dashboard</a>

	<div class="page-header">
		<h1>Portal Lockouts</h1>
		<p>Participants marked inactive (active = 'I') who are still in active pools</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else}
		<div class="toolbar">
			<span class="count-label">{rows.length} locked-out participant{rows.length !== 1 ? 's' : ''}</span>
			{#if rows.length > 0}
				<input
					class="search-input"
					type="search"
					placeholder="Search by name or part no…"
					bind:value={search}
				/>
			{/if}
		</div>

		{#if rows.length === 0}
			<div class="card empty">
				<span class="ok-icon">✓</span>
				<p>No portal lockouts.</p>
			</div>
		{:else}
			<div class="table-wrap">
				<table>
					<thead>
						<tr>
							<th>Part No</th>
							<th>First</th>
							<th>Last</th>
							<th></th>
						</tr>
					</thead>
					<tbody>
						{#each filtered as row (row.part_no)}
							<tr class:fading={rowStates[row.part_no] === 'ok'}>
								<td>{row.part_no}</td>
								<td>{row.fname ?? '—'}</td>
								<td>{row.lname ?? '—'}</td>
								<td class="action-cell">
									{#if rowStates[row.part_no] === 'pending'}
										<span class="text-muted">Unlocking…</span>
									{:else if rowStates[row.part_no] === 'ok'}
										<span class="msg-ok">Unlocked</span>
									{:else if rowStates[row.part_no] === 'err'}
										<span class="error" title={rowMessages[row.part_no]}>Failed</span>
									{:else}
										<button class="btn btn-primary btn-sm" onclick={() => openConfirm(row)}>
											Unlock
										</button>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	{/if}
</div>

{#if confirmRow}
	<div class="modal-overlay" role="dialog" aria-modal="true">
		<div class="modal-content">
			<div class="modal-header">
				<h2>Unlock Participant</h2>
				<button class="modal-close" onclick={closeConfirm} aria-label="Close">×</button>
			</div>
			<div class="modal-body">
				<p>
					Set <strong>{confirmRow.fname} {confirmRow.lname}</strong>
					(Part #{confirmRow.part_no}) back to active?
				</p>
				<p class="text-muted" style="font-size: 0.88rem;">
					This sets <code>participant.active = 'A'</code> in Informix immediately.
				</p>
				<div class="modal-actions">
					<button class="btn btn-primary" onclick={doUnlock}>Unlock</button>
					<button class="btn btn-secondary" onclick={closeConfirm}>Cancel</button>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.75rem;
	}

	.action-cell {
		text-align: right;
		white-space: nowrap;
	}

	.btn-sm {
		padding: 0.3rem 0.7rem;
		font-size: 0.82rem;
	}

	.fading {
		opacity: 0.4;
		transition: opacity 0.8s;
	}

	.card.empty {
		text-align: center;
		padding: 2.5rem;
		color: var(--text-muted);
	}

	.ok-icon {
		font-size: 2rem;
		color: var(--green);
		display: block;
		margin-bottom: 0.5rem;
	}

	.msg-ok {
		color: var(--green);
		font-weight: 600;
	}
</style>
