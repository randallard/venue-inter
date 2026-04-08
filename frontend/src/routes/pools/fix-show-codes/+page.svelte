<script lang="ts">
	import { onMount } from 'svelte';
	import { getBadShowCodes, getShowTypes, fixShowCode } from '$lib/api';
	import type { BadShowCodeRow, ShowTypeRow } from '$lib/types';

	let rows = $state<BadShowCodeRow[]>([]);
	let showTypes = $state<ShowTypeRow[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);

	// Group rows by pool_no so we can fix an entire pool at once
	let byPool = $derived(
		rows.reduce<Record<number, BadShowCodeRow[]>>((acc, r) => {
			(acc[r.pool_no] ??= []).push(r);
			return acc;
		}, {})
	);

	// Modal state
	let modal = $state<{ pool_no: number; new_code: string } | null>(null);
	let fixing = $state(false);
	let actionMsg = $state<{ ok: boolean; text: string } | null>(null);

	function openModal(pool_no: number) {
		modal = { pool_no, new_code: '' };
		actionMsg = null;
	}

	function closeModal() {
		modal = null;
	}

	async function submitFix() {
		if (!modal || !modal.new_code) return;
		fixing = true;
		actionMsg = null;
		try {
			const res = await fixShowCode({ pool_no: modal.pool_no, new_code: modal.new_code });
			actionMsg = { ok: res.ok, text: res.message };
			if (res.ok) {
				// Remove fixed pool from the list
				rows = rows.filter((r) => r.pool_no !== modal!.pool_no);
				modal = null;
			}
		} catch (e) {
			actionMsg = { ok: false, text: e instanceof Error ? e.message : 'Request failed' };
		} finally {
			fixing = false;
		}
	}

	async function load() {
		loading = true;
		error = null;
		try {
			const [data, types] = await Promise.all([getBadShowCodes(), getShowTypes()]);
			rows = data.rows;
			showTypes = types.rows;
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
		<h1>Fix Show Codes</h1>
		<p>Pools with unrecognized div_code values — must be corrected before export</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if Object.keys(byPool).length === 0}
		<div class="card empty">
			<span class="ok-icon">✓</span>
			<p>All show codes are valid.</p>
		</div>
	{:else}
		{#each Object.entries(byPool) as [pool_no_str, members]}
			{@const pool_no = Number(pool_no_str)}
			{@const bad_code = members[0].bad_code ?? '(blank)'}
			<div class="pool-block">
				<div class="pool-header">
					<div>
						<span class="pool-label">Pool #{pool_no}</span>
						<span class="bad-code-badge">{bad_code}</span>
					</div>
					<button class="btn btn-primary" onclick={() => openModal(pool_no)}>
						Fix Code
					</button>
				</div>
				<div class="table-wrap">
					<table>
						<thead>
							<tr>
								<th>PM ID</th>
								<th>Part No</th>
								<th>First</th>
								<th>Last</th>
								<th>Bad Code</th>
							</tr>
						</thead>
						<tbody>
							{#each members as m}
								<tr>
									<td>{m.pm_id}</td>
									<td>{m.part_no}</td>
									<td>{m.fname ?? '—'}</td>
									<td>{m.lname ?? '—'}</td>
									<td><span class="bad-code-badge">{m.bad_code ?? '(blank)'}</span></td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</div>
		{/each}
	{/if}
</div>

{#if modal}
	<div class="modal-overlay" role="dialog" aria-modal="true">
		<div class="modal-content">
			<div class="modal-header">
				<h2>Fix Show Code — Pool #{modal.pool_no}</h2>
				<button class="modal-close" onclick={closeModal} aria-label="Close">×</button>
			</div>
			<div class="modal-body">
				<p>Select the correct show type for pool <strong>#{modal.pool_no}</strong>:</p>
				<select bind:value={modal.new_code} style="width:100%; margin-bottom: 0.5rem;">
					<option value="">— select show type —</option>
					{#each showTypes as st}
						<option value={st.st_code}>{st.st_code} — {st.st_description}</option>
					{/each}
				</select>

				{#if actionMsg}
					<p class={actionMsg.ok ? 'msg-ok' : 'error'}>{actionMsg.text}</p>
				{/if}

				<div class="modal-actions">
					<button class="btn btn-primary" onclick={submitFix} disabled={fixing || !modal.new_code}>
						{fixing ? 'Saving…' : 'Apply Fix'}
					</button>
					<button class="btn btn-secondary" onclick={closeModal} disabled={fixing}>Cancel</button>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.pool-block {
		margin-bottom: 1.5rem;
	}

	.pool-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.5rem;
	}

	.pool-label {
		font-weight: 700;
		font-size: 1rem;
		color: var(--navy);
		margin-right: 0.6rem;
	}

	.bad-code-badge {
		display: inline-block;
		padding: 0.15rem 0.5rem;
		background: #fef2f2;
		color: #991b1b;
		border-radius: 3px;
		font-size: 0.8rem;
		font-weight: 700;
		font-family: monospace;
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
