<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { getReviewDetail, ceoDecide, listDocuments } from '$lib/api';
	import type { ReviewDetail, DocumentsResponse, DocumentMeta } from '$lib/types';

	const part_key = $derived($page.params.part_key ?? '');

	let detail = $state<ReviewDetail | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	// CEO decision state
	let ceoNotes = $state('');
	let deciding = $state(false);
	let inlineError = $state<string | null>(null);

	// Documents
	let docs = $state<DocumentsResponse | null>(null);
	let docsLoading = $state(true);
	let viewingDoc = $state<DocumentMeta | null>(null);

	async function loadDocs() {
		docsLoading = true;
		try {
			docs = await listDocuments(part_key);
		} catch {
			docs = null;
		} finally {
			docsLoading = false;
		}
	}

	const hasPending = $derived(docs?.documents.some(d => d.fetch_status === 'pending') ?? false);

	async function load() {
		loading = true;
		error = null;
		try {
			detail = await getReviewDetail(part_key);
			// Pre-fill notes if CEO already wrote some (e.g. page reload)
			if (detail.ceo_notes) ceoNotes = detail.ceo_notes;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load case';
		} finally {
			loading = false;
		}
	}

	async function decide(action: string) {
		if (!ceoNotes.trim()) {
			inlineError = 'Notes are required before making a decision.';
			return;
		}
		deciding = true;
		inlineError = null;

		// Timeout-with-refetch pattern for the async write guarantee
		const timeoutMs = 8000;
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), timeoutMs);

		try {
			const res = await ceoDecide({ part_key, action, notes: ceoNotes });
			clearTimeout(timer);

			if (res.ok) {
				// Success — navigate back to queue
				goto('/reviews/ceo');
				return;
			}
			inlineError = res.message;
		} catch (e: unknown) {
			clearTimeout(timer);
			const isAbort = e instanceof Error && e.name === 'AbortError';
			const isNet = e instanceof TypeError;

			if (isAbort || isNet) {
				// Network/timeout: re-fetch to check if decision was durably written
				try {
					const refreshed = await getReviewDetail(part_key);
					if (
						refreshed.pg_status === 'completed' ||
						refreshed.pg_status === 'sent_back'
					) {
						// Decision was committed — treat as success
						goto('/reviews/ceo');
						return;
					}
				} catch {
					// ignore — fall through to show inline error
				}
				inlineError = 'Network timeout. Please try again — your previous click may or may not have been saved.';
			} else {
				inlineError = e instanceof Error ? e.message : 'Request failed';
			}
		} finally {
			deciding = false;
		}
	}

	const memberStatusLabel: Record<number, string> = {
		1: 'In Pool', 2: 'Qualified', 5: 'Perm Excuse', 6: 'Disqualified', 7: 'Temp Excuse'
	};
	const memberStatusClass: Record<number, string> = {
		1: 'status-in-pool', 2: 'status-qualified', 5: 'status-perm-excuse',
		6: 'status-disqualified', 7: 'status-temp-excuse'
	};

	const alreadyDecided = $derived(
		detail?.pg_status === 'completed' || detail?.pg_status === 'sent_back'
	);

	onMount(() => { load(); loadDocs(); });
</script>

<div class="ceo-container">
	<div class="ceo-header">
		<a class="back-link-white" href="/reviews/ceo">← Back to Queue</a>
	</div>

	{#if loading}
		<div class="ceo-body">
			<p class="text-muted">Loading case…</p>
		</div>
	{:else if error}
		<div class="ceo-body">
			<p class="error">{error}</p>
		</div>
	{:else if detail}
		<div class="ceo-body">
			<!-- Case header -->
			<div class="case-header">
				<div>
					<h1>{detail.lname ?? '—'}, {detail.fname ?? '—'}</h1>
					<p class="case-meta">
						{detail.review_type.toUpperCase()} · Part #{detail.part_no} · Pool #{detail.pool_no}
						{#if detail.pool_ret_date} · {detail.pool_ret_date}{/if}
					</p>
				</div>
				{#if alreadyDecided}
					<div class="decided-badge">
						{detail.pg_status === 'completed' ? `Decided: ${detail.decision}` : 'Sent back to admin'}
					</div>
				{/if}
			</div>

			<div class="case-grid">
				<!-- Left: participant info -->
				<div class="info-stack">
					<div class="card">
						<h2>Participant</h2>
						<table class="detail-table"><tbody>
							<tr><td>Name</td><td>{detail.fname ?? '—'} {detail.lname ?? '—'}</td></tr>
							<tr><td>Address</td><td>{[detail.addr, detail.city, detail.state_code, detail.zip].filter(Boolean).join(', ') || '—'}</td></tr>
							<tr><td>Email</td><td>{detail.email ?? '—'}</td></tr>
							<tr><td>Gender</td><td>{detail.gender ?? '—'}</td></tr>
							<tr><td>Race</td><td>{detail.race_code ?? '—'}</td></tr>
						</tbody></table>
					</div>

					<div class="card">
						<h2>Current Status</h2>
						<table class="detail-table"><tbody>
							<tr>
								<td>Pool Status</td>
								<td>
									<span class="status-badge {memberStatusClass[detail.member_status] ?? ''}">
										{memberStatusLabel[detail.member_status] ?? detail.member_status}
									</span>
								</td>
							</tr>
							<tr><td>Submitted</td><td>{detail.submitted_date ?? '—'}</td></tr>
							{#if detail.sent_to_ceo_at}
								<tr><td>Sent to you</td><td>{new Date(detail.sent_to_ceo_at).toLocaleDateString()}</td></tr>
							{/if}
						</tbody></table>
					</div>

					{#if detail.admin_notes}
						<div class="card">
							<h2>Admin Notes</h2>
							<p class="notes-text">{detail.admin_notes}</p>
						</div>
					{/if}

					<!-- Documents -->
					<div class="card">
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
								<p class="scan-badge">
									{docs.scan_code === 'web'
										? 'Submitted online'
										: `Scan batch: ${docs.scan_code}`}
								</p>
							{/if}
							<ul class="doc-list">
								{#each docs.documents as doc}
									<li class="doc-row">
										<span class="doc-name">{doc.file_name}</span>
										{#if doc.fetch_status === 'cached'}
											<button
												class="btn-view"
												onclick={() => viewingDoc = doc}
											>View</button>
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
				</div>

				<!-- Right: decision panel -->
				<div class="decision-panel">
					{#if alreadyDecided}
						<div class="card decided-card">
							<h2>Decision Recorded</h2>
							{#if detail.decision}
								<p class="decision-label">{detail.decision.replace(/_/g, ' ')}</p>
							{:else}
								<p class="decision-label">Sent back to admin</p>
							{/if}
							{#if detail.ceo_notes}
								<p class="notes-text" style="margin-top: 0.5rem;">{detail.ceo_notes}</p>
							{/if}
							{#if detail.decided_at}
								<p class="text-muted" style="font-size: 0.82rem; margin-top: 0.5rem;">
									{new Date(detail.decided_at).toLocaleString()}
								</p>
							{/if}
						</div>
					{:else}
						<div class="card decision-card">
							<h2>Your Decision</h2>

							<label class="notes-label" for="ceo-notes">Notes <span class="req">*</span></label>
							<textarea
								id="ceo-notes"
								class="notes-area"
								bind:value={ceoNotes}
								placeholder="Required — document your reasoning"
								rows="5"
								disabled={deciding}
							></textarea>

							{#if inlineError}
								<p class="inline-error">{inlineError}</p>
							{/if}

							<div class="decision-buttons">
								<button
									class="btn btn-decision btn-requalify"
									onclick={() => decide('requalify')}
									disabled={deciding}
									title="Set pool member status to Qualified (2)"
								>
									Re-qualify
								</button>

								<button
									class="btn btn-decision btn-disqualify"
									onclick={() => decide('disqualify')}
									disabled={deciding}
									title="Set pool member status to Disqualified (6)"
								>
									Disqualify
								</button>

								<button
									class="btn btn-decision btn-perm"
									onclick={() => decide('permanent_excuse')}
									disabled={deciding}
									title="Set pool member status to Permanently Excused (5)"
								>
									Perm Excuse
								</button>

								<button
									class="btn btn-decision btn-temp"
									onclick={() => decide('temporary_excuse')}
									disabled={deciding}
									title="Set pool member status to Temporarily Excused (7)"
								>
									Temp Excuse
								</button>

								<button
									class="btn btn-decision btn-sendback"
									onclick={() => decide('send_back')}
									disabled={deciding}
									title="Return to admin queue for additional review"
								>
									Send Back
								</button>
							</div>

							{#if deciding}
								<p class="text-muted deciding-msg">Recording decision…</p>
							{/if}
						</div>
					{/if}
				</div>
			</div>
		</div>
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
			<img
				src="/api/documents/{viewingDoc.id}"
				alt={viewingDoc.file_name}
				class="doc-img"
			/>
		</div>
	</div>
{/if}

<style>
	/* CEO view has its own full-page layout — no standard container */
	.ceo-container {
		min-height: 100vh;
		background: var(--bg);
	}

	.ceo-header {
		background: var(--navy);
		padding: 0.75rem 2rem;
		display: flex;
		align-items: center;
	}

	.back-link-white {
		color: rgba(255,255,255,0.8);
		text-decoration: none;
		font-size: 0.9rem;
		font-weight: 600;
	}
	.back-link-white:hover { color: #fff; text-decoration: none; }

	.ceo-body {
		max-width: 1100px;
		margin: 0 auto;
		padding: 1.5rem 2rem;
	}

	.case-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		margin-bottom: 1.25rem;
	}
	.case-header h1 { margin: 0 0 0.2rem; font-size: 1.6rem; }
	.case-meta { color: var(--text-muted); font-size: 0.92rem; margin: 0; }

	.decided-badge {
		background: var(--green);
		color: #fff;
		padding: 0.35rem 0.85rem;
		border-radius: var(--radius-sm);
		font-weight: 700;
		font-size: 0.88rem;
		text-transform: capitalize;
		white-space: nowrap;
	}

	.case-grid {
		display: grid;
		grid-template-columns: 1fr 420px;
		gap: 1.25rem;
		align-items: start;
	}

	@media (max-width: 900px) {
		.case-grid { grid-template-columns: 1fr; }
	}

	.info-stack {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.notes-text {
		color: var(--text-muted);
		font-size: 0.9rem;
		margin: 0;
		line-height: 1.5;
	}

	/* Decision panel */
	.decision-card { position: sticky; top: 70px; }

	.notes-label {
		display: block;
		font-weight: 600;
		font-size: 0.88rem;
		margin-bottom: 0.4rem;
	}
	.req { color: var(--red); }

	.notes-area {
		width: 100%;
		padding: 0.5rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-family: inherit;
		font-size: 0.9rem;
		resize: vertical;
		background: var(--surface);
		margin-bottom: 0.75rem;
	}
	.notes-area:focus {
		outline: none;
		border-color: var(--gold);
		box-shadow: 0 0 0 3px rgba(181,152,90,0.15);
	}

	.inline-error {
		color: var(--red);
		font-size: 0.88rem;
		font-weight: 600;
		margin: 0 0 0.75rem;
	}

	.decision-buttons {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.6rem;
	}

	.btn-decision {
		padding: 0.6rem 0.75rem;
		font-size: 0.88rem;
		font-weight: 700;
		border-radius: var(--radius-sm);
		border: none;
		cursor: pointer;
		transition: opacity 0.15s, transform 0.1s;
	}
	.btn-decision:disabled { opacity: 0.45; cursor: not-allowed; transform: none; }
	.btn-decision:not(:disabled):active { transform: translateY(1px); }

	.btn-requalify  { background: #dcfce7; color: #166534; }
	.btn-requalify:not(:disabled):hover  { background: #bbf7d0; }

	.btn-disqualify { background: #fef2f2; color: #991b1b; }
	.btn-disqualify:not(:disabled):hover { background: #fee2e2; }

	.btn-perm       { background: #f1f5f9; color: #475569; }
	.btn-perm:not(:disabled):hover       { background: #e2e8f0; }

	.btn-temp       { background: #fef9c3; color: #854d0e; }
	.btn-temp:not(:disabled):hover       { background: #fef08a; }

	.btn-sendback   { grid-column: 1 / -1; background: var(--navy); color: #fff; }
	.btn-sendback:not(:disabled):hover   { background: var(--navy-light); }

	.deciding-msg {
		text-align: center;
		font-size: 0.88rem;
		margin-top: 0.75rem;
		margin-bottom: 0;
	}

	/* Decided state */
	.decided-card { border-left: 4px solid var(--green); }
	.decision-label {
		font-size: 1.25rem;
		font-weight: 700;
		color: var(--navy);
		text-transform: capitalize;
	}

	/* Documents card */
	.docs-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.6rem;
	}
	.docs-header h2 { margin: 0; }

	.btn-refresh {
		font-size: 0.8rem;
		padding: 0.2rem 0.6rem;
		background: none;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		cursor: pointer;
		color: var(--text-muted);
	}
	.btn-refresh:hover { border-color: var(--navy); color: var(--navy); }

	.scan-badge {
		font-size: 0.8rem;
		color: var(--gold);
		font-weight: 600;
		margin: 0 0 0.6rem;
	}

	.doc-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}
	.doc-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.35rem 0;
		border-bottom: 1px solid var(--border);
	}
	.doc-row:last-child { border-bottom: none; }
	.doc-name {
		font-size: 0.85rem;
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.btn-view {
		flex-shrink: 0;
		font-size: 0.8rem;
		padding: 0.2rem 0.65rem;
		background: var(--navy);
		color: #fff;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	.btn-view:hover { background: var(--navy-light); }

	.doc-status {
		flex-shrink: 0;
		font-size: 0.78rem;
		font-weight: 600;
	}
	.doc-status.pending { color: var(--gold); }
	.doc-status.failed  { color: var(--red); }

	/* Image viewer modal */
	.doc-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0,0,0,0.75);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}
	.doc-viewer {
		background: #fff;
		border-radius: var(--radius);
		overflow: hidden;
		max-width: 90vw;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		box-shadow: 0 8px 32px rgba(0,0,0,0.4);
	}
	.doc-viewer-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.6rem 1rem;
		background: var(--navy);
		color: #fff;
	}
	.doc-viewer-name {
		font-size: 0.88rem;
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.doc-close {
		background: none;
		border: none;
		color: rgba(255,255,255,0.8);
		font-size: 1.1rem;
		cursor: pointer;
		padding: 0 0.25rem;
		flex-shrink: 0;
	}
	.doc-close:hover { color: #fff; }
	.doc-img {
		max-width: 85vw;
		max-height: calc(90vh - 44px);
		object-fit: contain;
		display: block;
	}
</style>
