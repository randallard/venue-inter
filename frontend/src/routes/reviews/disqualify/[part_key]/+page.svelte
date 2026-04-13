<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getReviewDetail, sendToCeo, listDocuments } from '$lib/api';
	import type { ReviewDetail, DocumentsResponse, DocumentMeta } from '$lib/types';

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
			if (res.ok) await load();
		} catch (e) {
			sendResult = { ok: false, msg: e instanceof Error ? e.message : 'Request failed' };
		} finally {
			sending = false;
		}
	}

	const memberStatusLabel: Record<number, string> = {
		1: 'In Pool', 2: 'Qualified', 5: 'Perm Excuse', 6: 'Disqualified', 7: 'Temp Excuse'
	};
	const memberStatusClass: Record<number, string> = {
		1: 'status-in-pool', 2: 'status-qualified', 5: 'status-perm-excuse',
		6: 'status-disqualified', 7: 'status-temp-excuse'
	};

	let docs = $state<DocumentsResponse | null>(null);
	let docsLoading = $state(true);
	let viewingDoc = $state<DocumentMeta | null>(null);

	async function loadDocs() {
		docsLoading = true;
		try { docs = await listDocuments(part_key); }
		catch { docs = null; }
		finally { docsLoading = false; }
	}

	const hasPending = $derived(docs?.documents.some(d => d.fetch_status === 'pending') ?? false);

	onMount(() => { load(); loadDocs(); });
</script>

<div class="container">
	<div class="nav-row">
		<a class="back-link" href="/reviews/disqualify">Disqualify Queue</a>
		{#if detail}
			<a class="back-link" href="/reviews/records/{detail.part_no}">View History</a>
		{/if}
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if detail}
		<div class="page-header">
			<h1>{detail.lname ?? '—'}, {detail.fname ?? '—'}</h1>
			<p>Disqualification review — Part #{detail.part_no} · Pool #{detail.pool_no}</p>
		</div>

		<div class="detail-grid">
			<div class="card">
				<h2>Participant</h2>
				<table class="detail-table"><tbody>
					<tr><td>Name</td><td>{detail.fname ?? '—'} {detail.lname ?? '—'}</td></tr>
					<tr><td>Part No</td><td>{detail.part_no}</td></tr>
					<tr><td>Address</td><td>{[detail.addr, detail.city, detail.state_code, detail.zip].filter(Boolean).join(', ') || '—'}</td></tr>
					<tr><td>Email</td><td>{detail.email ?? '—'}</td></tr>
					<tr><td>Active</td><td>{detail.active ?? '—'}</td></tr>
				</tbody></table>
			</div>

			<div class="card">
				<h2>Pool Status</h2>
				<table class="detail-table"><tbody>
					<tr><td>Pool</td><td>#{detail.pool_no}</td></tr>
					<tr><td>Return Date</td><td>{detail.pool_ret_date ?? '—'}</td></tr>
					<tr>
						<td>Status</td>
						<td><span class="status-badge {memberStatusClass[detail.member_status] ?? ''}">{memberStatusLabel[detail.member_status] ?? detail.member_status}</span></td>
					</tr>
					{#if detail.decided_at}
						<tr><td>Decided</td><td>{new Date(detail.decided_at).toLocaleString()}</td></tr>
					{/if}
					{#if detail.decision}
						<tr><td>Decision</td><td><strong>{detail.decision}</strong></td></tr>
					{/if}
				</tbody></table>
			</div>
		</div>

		<div class="card" style="margin-top: 1rem;">
			<h2>Review Record</h2>
			<table class="detail-table"><tbody>
				<tr><td>Type</td><td>{detail.review_type}</td></tr>
				<tr><td>Submitted</td><td>{detail.submitted_date ?? '—'}</td></tr>
				<tr><td>IFX Status</td><td>{detail.ifx_status}</td></tr>
				{#if detail.pg_status}
					<tr><td>Queue Status</td><td><span class="pg-status">{detail.pg_status}</span></td></tr>
				{/if}
				{#if detail.sent_to_ceo_at}
					<tr><td>Sent to CEO</td><td>{new Date(detail.sent_to_ceo_at).toLocaleString()}</td></tr>
				{/if}
			</tbody></table>
		</div>

		<!-- Documents -->
		<div class="card" style="margin-top: 1rem;">
			<div class="docs-header">
				<h2>Documents</h2>
				{#if hasPending}
					<button class="btn-refresh" onclick={loadDocs}>Refresh</button>
				{/if}
			</div>
			{#if docsLoading}
				<p class="text-muted" style="font-size:0.88rem">Loading…</p>
			{:else if !docs || docs.documents.length === 0}
				<p class="text-muted" style="font-size:0.88rem">No documents on file.</p>
			{:else}
				{#if docs.scan_code}
					<p class="scan-badge">{docs.scan_code === 'web' ? 'Submitted online' : `Scan batch: ${docs.scan_code}`}</p>
				{/if}
				<ul class="doc-list">
					{#each docs.documents as doc}
						<li class="doc-row">
							<span class="doc-name">{doc.file_name}</span>
							{#if doc.fetch_status === 'cached'}
								<button class="btn-view" onclick={() => viewingDoc = doc}>View</button>
							{:else if doc.fetch_status === 'pending'}
								<span class="doc-status pending">Fetching…</span>
							{:else}
								<span class="doc-status failed">Unavailable</span>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		{#if detail.pg_status !== 'completed'}
			<div class="card" style="margin-top: 1rem;">
				<h2>Admin Notes</h2>
				<textarea class="notes-area" bind:value={adminNotes} placeholder="Notes for the CEO (optional)" rows="4"></textarea>
				{#if sendResult}
					<p class={sendResult.ok ? 'msg-ok' : 'error'} style="margin-top: 0.5rem;">{sendResult.msg}</p>
				{/if}
				<div style="margin-top: 0.75rem;">
					<button class="btn btn-primary" onclick={doSendToCeo} disabled={sending}>
						{sending ? 'Sending…' : detail.pg_status === 'pending_ceo' ? 'Update & Re-send to CEO' : 'Send to CEO'}
					</button>
				</div>
			</div>
		{:else}
			<div class="card" style="margin-top: 1rem; border-left: 4px solid var(--green);">
				<p style="margin: 0; font-weight: 600; color: var(--green);">
					Decided: <em>{detail.decision}</em>
				</p>
			</div>
		{/if}
	{/if}
</div>

{#if viewingDoc}
	<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
	<div class="doc-overlay" onclick={() => viewingDoc = null}>
		<div class="doc-viewer" onclick={(e) => e.stopPropagation()}>
			<div class="doc-viewer-header">
				<span class="doc-viewer-name">{viewingDoc.file_name}</span>
				<button class="doc-close" onclick={() => viewingDoc = null}>✕</button>
			</div>
			<img src="/api/documents/{viewingDoc.id}" alt={viewingDoc.file_name} class="doc-img" />
		</div>
	</div>
{/if}

<style>
	.detail-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
	@media (max-width: 700px) { .detail-grid { grid-template-columns: 1fr; } }
	.notes-area { width: 100%; padding: 0.5rem 0.75rem; border: 1px solid var(--border); border-radius: var(--radius-sm); font-family: inherit; font-size: 0.9rem; resize: vertical; background: var(--surface); }
	.notes-area:focus { outline: none; border-color: var(--gold); box-shadow: 0 0 0 3px rgba(181,152,90,0.15); }
	.pg-status { background: #dbeafe; color: #1e40af; padding: 0.15rem 0.5rem; border-radius: 3px; font-size: 0.82rem; font-weight: 600; }
	.msg-ok { color: var(--green); font-weight: 600; }
	.nav-row { display: flex; gap: 1rem; align-items: center; margin-bottom: 0.5rem; }

	.docs-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.6rem; }
	.docs-header h2 { margin: 0; }
	.btn-refresh { font-size: 0.8rem; padding: 0.2rem 0.6rem; background: none; border: 1px solid var(--border); border-radius: var(--radius-sm); cursor: pointer; color: var(--text-muted); }
	.btn-refresh:hover { border-color: var(--navy); color: var(--navy); }
	.scan-badge { font-size: 0.8rem; color: var(--gold); font-weight: 600; margin: 0 0 0.6rem; }
	.doc-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.4rem; }
	.doc-row { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; padding: 0.35rem 0; border-bottom: 1px solid var(--border); }
	.doc-row:last-child { border-bottom: none; }
	.doc-name { font-size: 0.85rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.btn-view { flex-shrink: 0; font-size: 0.8rem; padding: 0.2rem 0.65rem; background: var(--navy); color: #fff; border: none; border-radius: var(--radius-sm); cursor: pointer; }
	.btn-view:hover { background: var(--navy-light); }
	.doc-status { flex-shrink: 0; font-size: 0.78rem; font-weight: 600; }
	.doc-status.pending { color: var(--gold); }
	.doc-status.failed { color: var(--red); }

	.doc-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.75); display: flex; align-items: center; justify-content: center; z-index: 1000; }
	.doc-viewer { background: #fff; border-radius: var(--radius); overflow: hidden; max-width: 90vw; max-height: 90vh; display: flex; flex-direction: column; box-shadow: 0 8px 32px rgba(0,0,0,0.4); }
	.doc-viewer-header { display: flex; align-items: center; justify-content: space-between; padding: 0.6rem 1rem; background: var(--navy); color: #fff; }
	.doc-viewer-name { font-size: 0.88rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.doc-close { background: none; border: none; color: rgba(255,255,255,0.8); font-size: 1.1rem; cursor: pointer; padding: 0 0.25rem; flex-shrink: 0; }
	.doc-close:hover { color: #fff; }
	.doc-img { max-width: 85vw; max-height: calc(90vh - 44px); object-fit: contain; display: block; }
</style>
