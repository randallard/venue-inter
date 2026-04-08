<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getReviewDetail, sendToCeo } from '$lib/api';
	import type { ReviewDetail } from '$lib/types';

	const part_key = $derived($page.params.part_key ?? '');

	let detail = $state<ReviewDetail | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	let adminNotes = $state('');
	let sending = $state(false);
	let sendResult = $state<{ ok: boolean; msg: string } | null>(null);

	async function load() {
		loading = true;
		error = null;
		try {
			detail = await getReviewDetail(part_key);
			adminNotes = detail.admin_notes ?? '';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	async function doSendToCeo() {
		if (!detail) return;
		sending = true;
		sendResult = null;
		try {
			const res = await sendToCeo({ part_key, admin_notes: adminNotes || null });
			sendResult = { ok: res.ok, msg: res.message };
			if (res.ok) {
				// Refresh to show updated status
				await load();
			}
		} catch (e) {
			sendResult = { ok: false, msg: e instanceof Error ? e.message : 'Request failed' };
		} finally {
			sending = false;
		}
	}

	const memberStatusLabel: Record<number, string> = {
		1: 'In Pool',
		2: 'Qualified',
		5: 'Perm Excuse',
		6: 'Disqualified',
		7: 'Temp Excuse'
	};

	const memberStatusClass: Record<number, string> = {
		1: 'status-in-pool',
		2: 'status-qualified',
		5: 'status-perm-excuse',
		6: 'status-disqualified',
		7: 'status-temp-excuse'
	};

	const alreadySent = $derived(
		detail?.pg_status === 'pending_ceo' || detail?.pg_status === 'completed' || detail?.pg_status === 'sent_back'
	);

	onMount(load);
</script>

<div class="container">
	<a class="back-link" href="/reviews/excuse">Excuse Queue</a>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if detail}
		<div class="page-header">
			<h1>{detail.lname ?? '—'}, {detail.fname ?? '—'}</h1>
			<p>
				Excuse review — Part #{detail.part_no} · Pool #{detail.pool_no}
				{#if detail.pool_ret_date}· {detail.pool_ret_date}{/if}
			</p>
		</div>

		<div class="detail-grid">
			<!-- Participant panel -->
			<div class="card">
				<h2>Participant</h2>
				<table class="detail-table">
					<tbody>
						<tr><td>Name</td><td>{detail.fname ?? '—'} {detail.lname ?? '—'}</td></tr>
						<tr><td>Part No</td><td>{detail.part_no}</td></tr>
						<tr><td>Address</td><td>{[detail.addr, detail.city, detail.state_code, detail.zip].filter(Boolean).join(', ') || '—'}</td></tr>
						<tr><td>Email</td><td>{detail.email ?? '—'}</td></tr>
						<tr><td>Gender</td><td>{detail.gender ?? '—'}</td></tr>
						<tr><td>Race</td><td>{detail.race_code ?? '—'}</td></tr>
						<tr><td>Active</td><td>{detail.active ?? '—'}</td></tr>
					</tbody>
				</table>
			</div>

			<!-- Pool & status panel -->
			<div class="card">
				<h2>Pool Status</h2>
				<table class="detail-table">
					<tbody>
						<tr><td>Pool</td><td>#{detail.pool_no}</td></tr>
						<tr><td>Show Type</td><td>{detail.pool_div_code ?? '—'}</td></tr>
						<tr><td>Return Date</td><td>{detail.pool_ret_date ?? '—'}</td></tr>
						<tr><td>PM ID</td><td>{detail.pm_id}</td></tr>
						<tr>
							<td>Status</td>
							<td>
								<span class="status-badge {memberStatusClass[detail.member_status] ?? ''}">
									{memberStatusLabel[detail.member_status] ?? detail.member_status}
								</span>
							</td>
						</tr>
					</tbody>
				</table>
			</div>
		</div>

		<!-- Review record -->
		<div class="card" style="margin-top: 1rem;">
			<h2>Review Record</h2>
			<table class="detail-table">
				<tbody>
					<tr><td>Type</td><td>{detail.review_type}</td></tr>
					<tr><td>Submitted</td><td>{detail.submitted_date ?? '—'}</td></tr>
					<tr><td>IFX Status</td><td>{detail.ifx_status}</td></tr>
					{#if detail.pg_status}
						<tr><td>Queue Status</td><td><span class="pg-status">{detail.pg_status}</span></td></tr>
					{/if}
					{#if detail.sent_to_ceo_at}
						<tr><td>Sent to CEO</td><td>{new Date(detail.sent_to_ceo_at).toLocaleString()}</td></tr>
					{/if}
					{#if detail.decided_at}
						<tr><td>Decided</td><td>{new Date(detail.decided_at).toLocaleString()}</td></tr>
					{/if}
					{#if detail.decision}
						<tr><td>Decision</td><td><strong>{detail.decision}</strong></td></tr>
					{/if}
				</tbody>
			</table>
		</div>

		<!-- Admin notes + send to CEO -->
		{#if detail.pg_status !== 'completed'}
			<div class="card action-card" style="margin-top: 1rem;">
				<h2>Admin Notes</h2>
				<textarea
					class="notes-area"
					bind:value={adminNotes}
					placeholder="Add notes for the CEO (optional)"
					rows="4"
					disabled={alreadySent && detail.pg_status !== 'sent_back'}
				></textarea>

				{#if sendResult}
					<p class={sendResult.ok ? 'msg-ok' : 'error'} style="margin-top: 0.5rem;">{sendResult.msg}</p>
				{/if}

				{#if detail.pg_status === 'pending_ceo'}
					<p class="text-muted" style="margin-top: 0.75rem; font-size: 0.88rem;">
						This record is in the CEO queue. Re-sending will update the notes.
					</p>
				{/if}

				<div class="action-row">
					<button
						class="btn btn-primary"
						onclick={doSendToCeo}
						disabled={sending}
					>
						{#if sending}
							Sending…
						{:else if detail.pg_status === 'pending_ceo'}
							Update & Re-send to CEO
						{:else}
							Send to CEO
						{/if}
					</button>
				</div>
			</div>
		{:else}
			<div class="card" style="margin-top: 1rem; border-left: 4px solid var(--green);">
				<p style="margin: 0; font-weight: 600; color: var(--green);">
					This review has been decided: <em>{detail.decision}</em>
				</p>
				{#if detail.ceo_notes}
					<p style="margin-top: 0.5rem; color: var(--text-muted); font-size: 0.9rem;">{detail.ceo_notes}</p>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style>
	.detail-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 1rem;
	}

	@media (max-width: 700px) {
		.detail-grid { grid-template-columns: 1fr; }
	}

	.notes-area {
		width: 100%;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-family: inherit;
		font-size: 0.9rem;
		resize: vertical;
		background: var(--surface);
	}
	.notes-area:focus {
		outline: none;
		border-color: var(--gold);
		box-shadow: 0 0 0 3px rgba(181,152,90,0.15);
	}

	.action-card h2 { margin-bottom: 0.75rem; }
	.action-row { margin-top: 0.75rem; }

	.pg-status {
		background: #dbeafe;
		color: #1e40af;
		padding: 0.15rem 0.5rem;
		border-radius: 3px;
		font-size: 0.82rem;
		font-weight: 600;
	}

	.msg-ok { color: var(--green); font-weight: 600; }
</style>
