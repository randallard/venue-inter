<script lang="ts">
	import { onMount } from 'svelte';
	import { getBlankQuestionnaires, resetQQ } from '$lib/api';
	import type { BlankQQRow } from '$lib/types';

	let rows = $state<BlankQQRow[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let search = $state('');

	let filtered = $derived(
		search.trim()
			? rows.filter((r) => {
					const q = search.toLowerCase();
					return (
						String(r.pm_id).includes(q) ||
						String(r.part_no).includes(q) ||
						(r.fname ?? '').toLowerCase().includes(q) ||
						(r.lname ?? '').toLowerCase().includes(q) ||
						String(r.pool_no).includes(q)
					);
			  })
			: rows
	);

	// Per-row state: 'idle' | 'pending' | 'ok' | 'err'
	type RowState = 'idle' | 'pending' | 'ok' | 'err';
	let rowStates = $state<Record<number, RowState>>({});
	let rowMessages = $state<Record<number, string>>({});

	// Confirmation modal
	let confirmRow = $state<BlankQQRow | null>(null);

	function openConfirm(row: BlankQQRow) {
		confirmRow = row;
	}

	function closeConfirm() {
		confirmRow = null;
	}

	async function doReset() {
		if (!confirmRow) return;
		const pm_id = confirmRow.pm_id;
		confirmRow = null;
		rowStates[pm_id] = 'pending';
		try {
			const res = await resetQQ(pm_id);
			rowStates[pm_id] = res.ok ? 'ok' : 'err';
			rowMessages[pm_id] = res.message;
			if (res.ok) {
				// Fade out after brief delay
				setTimeout(() => {
					rows = rows.filter((r) => r.pm_id !== pm_id);
				}, 1200);
			}
		} catch (e) {
			rowStates[pm_id] = 'err';
			rowMessages[pm_id] = e instanceof Error ? e.message : 'Reset failed';
		}
	}

	async function load() {
		loading = true;
		error = null;
		try {
			const data = await getBlankQuestionnaires();
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
		<h1>Blank Questionnaires</h1>
		<p>Pool members in active pools who have not responded to their questionnaire</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else}
		<div class="toolbar">
			<span class="count-label">{rows.length} member{rows.length !== 1 ? 's' : ''} with blank QQ</span>
			{#if rows.length > 0}
				<input
					class="search-input"
					type="search"
					placeholder="Search by name, pool, part…"
					bind:value={search}
				/>
			{/if}
		</div>

		{#if rows.length === 0}
			<div class="card empty">
				<span class="ok-icon">✓</span>
				<p>No blank questionnaires.</p>
			</div>
		{:else}
			<div class="table-wrap">
				<table>
					<thead>
						<tr>
							<th>PM ID</th>
							<th>Pool</th>
							<th>Part No</th>
							<th>First</th>
							<th>Last</th>
							<th>Return Date</th>
							<th></th>
						</tr>
					</thead>
					<tbody>
						{#each filtered as row (row.pm_id)}
							<tr class:fading={rowStates[row.pm_id] === 'ok'}>
								<td>{row.pm_id}</td>
								<td>{row.pool_no}</td>
								<td>{row.part_no}</td>
								<td>{row.fname ?? '—'}</td>
								<td>{row.lname ?? '—'}</td>
								<td>{row.ret_date ?? '—'}</td>
								<td class="action-cell">
									{#if rowStates[row.pm_id] === 'pending'}
										<span class="text-muted">Resetting…</span>
									{:else if rowStates[row.pm_id] === 'ok'}
										<span class="msg-ok">Reset</span>
									{:else if rowStates[row.pm_id] === 'err'}
										<span class="error" title={rowMessages[row.pm_id]}>Failed</span>
									{:else}
										<button class="btn btn-primary btn-sm" onclick={() => openConfirm(row)}>
											Reset QQ
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
				<h2>Reset Questionnaire</h2>
				<button class="modal-close" onclick={closeConfirm} aria-label="Close">×</button>
			</div>
			<div class="modal-body">
				<p>
					Reset the questionnaire for <strong>{confirmRow.fname} {confirmRow.lname}</strong>
					(PM ID {confirmRow.pm_id}, Pool #{confirmRow.pool_no})?
				</p>
				<p class="text-muted" style="font-size: 0.88rem;">
					This sets responded = 'N' and clears scan_code. The member will need to re-submit.
				</p>
				<div class="modal-actions">
					<button class="btn btn-danger" onclick={doReset}>Reset</button>
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
