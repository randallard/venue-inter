<script lang="ts">
	import { onMount } from 'svelte';
	import { getCeoQueue, getReviewDetail, ceoDecide } from '$lib/api';
	import type { CeoReviewRow, ReviewDetail } from '$lib/types';

	type Case = {
		row: CeoReviewRow;
		detail: ReviewDetail | null;
		loading: boolean;
		error: string | null;
		notes: string;
		deciding: boolean;
		formError: string | null;
	};

	let cases = $state<Case[]>([]);
	let queueLoading = $state(true);
	let queueError = $state<string | null>(null);
	let maintenance = $state(false);

	async function load() {
		queueLoading = true;
		queueError = null;
		try {
			const data = await getCeoQueue();
			maintenance = data.maintenance;
			cases = data.rows.map((row) => ({
				row,
				detail: null,
				loading: true,
				error: null,
				notes: '',
				deciding: false,
				formError: null
			}));
			// Fetch all details in parallel — each updates its slot as it resolves
			data.rows.forEach((row, i) => {
				getReviewDetail(row.part_key)
					.then((detail) => {
						cases[i] = {
							...cases[i],
							detail,
							loading: false,
							notes: detail.ceo_notes ?? ''
						};
					})
					.catch((e) => {
						cases[i] = {
							...cases[i],
							error: e instanceof Error ? e.message : 'Failed to load',
							loading: false
						};
					});
			});
		} catch (e) {
			queueError = e instanceof Error ? e.message : 'Failed to load queue';
		} finally {
			queueLoading = false;
		}
	}

	async function decide(i: number, action: string) {
		const c = cases[i];
		if (!c.notes.trim()) {
			cases[i] = { ...cases[i], formError: 'Notes are required before making a decision.' };
			return;
		}
		cases[i] = { ...cases[i], deciding: true, formError: null };

		const part_key = c.row.part_key;
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), 8000);

		try {
			const res = await ceoDecide({ part_key, action, notes: c.notes });
			clearTimeout(timer);
			if (res.ok) {
				cases[i] = {
					...cases[i],
					deciding: false,
					detail: {
						...cases[i].detail!,
						pg_status: res.status,
						decision: res.decision,
						ceo_notes: c.notes,
						decided_at: new Date().toISOString()
					}
				};
			} else {
				cases[i] = { ...cases[i], deciding: false, formError: res.message };
			}
		} catch (e: unknown) {
			clearTimeout(timer);
			const isAbort = e instanceof Error && e.name === 'AbortError';
			const isNet = e instanceof TypeError;
			if (isAbort || isNet) {
				try {
					const refreshed = await getReviewDetail(part_key);
					if (refreshed.pg_status === 'completed' || refreshed.pg_status === 'sent_back') {
						cases[i] = { ...cases[i], deciding: false, detail: refreshed };
						return;
					}
				} catch {
					/* ignore */
				}
				cases[i] = {
					...cases[i],
					deciding: false,
					formError: 'Network timeout — please try again.'
				};
			} else {
				cases[i] = {
					...cases[i],
					deciding: false,
					formError: e instanceof Error ? e.message : 'Request failed'
				};
			}
		}
	}

	const memberStatusLabel: Record<number, string> = {
		1: 'In Pool',
		2: 'Qualified',
		5: 'Perm Excuse',
		6: 'Disqualified',
		7: 'Temp Excuse'
	};

	function formatDate(iso: string | null) {
		if (!iso) return '—';
		return new Date(iso).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function decisionLabel(d: string | null, status: string | null) {
		if (status === 'sent_back') return 'Sent back to admin';
		if (!d) return '—';
		return d.replace(/_/g, ' ');
	}

	const pendingCount = $derived(
		cases.filter(
			(c) =>
				c.detail?.pg_status !== 'completed' && c.detail?.pg_status !== 'sent_back' && !c.loading
		).length
	);

	onMount(load);
</script>

<div class="page-wrap">
	<div class="page-header">
		<div>
			<h1>Review Queue</h1>
			{#if !queueLoading && !queueError && !maintenance}
				<p class="subhead">
					{#if cases.length === 0}
						No cases pending your review.
					{:else}
						{pendingCount} of {cases.length} awaiting decision
					{/if}
				</p>
			{/if}
		</div>
		<button class="btn btn-secondary btn-sm" onclick={load} disabled={queueLoading}>
			Refresh
		</button>
	</div>

	{#if queueLoading}
		<p class="text-muted loading-msg">Loading…</p>
	{:else if queueError}
		<p class="error">{queueError}</p>
	{:else if maintenance}
		<div class="card maintenance-card">
			<div class="maint-icon">⚙</div>
			<h2>Queue in Maintenance Mode</h2>
			<p>The review queue is temporarily unavailable. Please check back shortly.</p>
		</div>
	{:else if cases.length === 0}
		<div class="card empty-card">
			<span class="ok-icon">✓</span>
			<p>No cases pending your review.</p>
		</div>
	{:else}
		<div class="case-list">
			{#each cases as c, i}
				{@const decided =
					c.detail?.pg_status === 'completed' || c.detail?.pg_status === 'sent_back'}

				{#if decided}
					<!-- Compact decided row -->
					<div class="case-decided">
						<div class="decided-info">
							<span class="decided-name">{c.row.lname ?? '—'}, {c.row.fname ?? '—'}</span>
							<span class="decided-meta">
								<span class="type-badge type-{c.row.review_type}">{c.row.review_type}</span>
								Pool #{c.row.pool_no}
							</span>
						</div>
						<div class="decided-result">
							<span class="decided-label {c.detail?.pg_status === 'sent_back' ? 'sent-back' : 'resolved'}">
								{decisionLabel(c.detail?.decision ?? null, c.detail?.pg_status ?? null)}
							</span>
							{#if c.detail?.decided_at}
								<span class="decided-at">{formatDate(c.detail.decided_at)}</span>
							{/if}
						</div>
					</div>
				{:else}
					<!-- Full expanded case card -->
					<div class="case-card" id="case-{c.row.part_key}">
						<div class="case-head">
							<div class="case-title">
								<span class="case-name">{c.row.lname ?? '—'}, {c.row.fname ?? '—'}</span>
								<span class="case-meta-head">
									<span class="type-badge type-{c.row.review_type}">{c.row.review_type}</span>
									Part #{c.row.part_no} · Pool #{c.row.pool_no}
									{#if c.detail?.pool_ret_date} · {c.detail.pool_ret_date}{/if}
									· Sent {formatDate(c.row.sent_to_ceo_at)}
								</span>
							</div>
						</div>

						{#if c.loading}
							<p class="text-muted case-loading">Loading case details…</p>
						{:else if c.error}
							<p class="error">{c.error}</p>
						{:else if c.detail}
							<div class="case-body">
								<!-- Left: info -->
								<div class="info-col">
									<div class="info-section">
										<div class="info-label">Participant</div>
										<table class="info-table">
											<tbody>
												<tr>
													<td>Address</td>
													<td
														>{[
															c.detail.addr,
															c.detail.city,
															c.detail.state_code,
															c.detail.zip
														]
															.filter(Boolean)
															.join(', ') || '—'}</td
													>
												</tr>
												<tr><td>Email</td><td>{c.detail.email ?? '—'}</td></tr>
												<tr><td>Gender</td><td>{c.detail.gender ?? '—'}</td></tr>
												<tr><td>Race</td><td>{c.detail.race_code ?? '—'}</td></tr>
											</tbody>
										</table>
									</div>

									<div class="info-section">
										<div class="info-label">Pool Status</div>
										<table class="info-table">
											<tbody>
												<tr>
													<td>Status</td>
													<td
														><span class="status-pill">
															{memberStatusLabel[c.detail.member_status] ??
																c.detail.member_status}
														</span></td
													>
												</tr>
												<tr
													><td>Submitted</td><td>{c.detail.submitted_date ?? '—'}</td></tr
												>
											</tbody>
										</table>
									</div>

									{#if c.detail.admin_notes}
										<div class="info-section admin-notes">
											<div class="info-label">Admin Notes</div>
											<p class="notes-text">{c.detail.admin_notes}</p>
										</div>
									{/if}
								</div>

								<!-- Right: decision -->
								<div class="decision-col">
									<div class="info-label">Your Decision</div>

									<label class="sr-only" for="notes-{c.row.part_key}">Notes</label>
									<textarea
										id="notes-{c.row.part_key}"
										class="notes-area"
										bind:value={cases[i].notes}
										placeholder="Required — document your reasoning"
										rows="4"
										disabled={c.deciding}
									></textarea>

									{#if c.formError}
										<p class="form-error">{c.formError}</p>
									{/if}

									<div class="decision-btns">
										<button
											class="btn-decide btn-requalify"
											onclick={() => decide(i, 'requalify')}
											disabled={c.deciding}
										>Re-qualify</button>

										<button
											class="btn-decide btn-disqualify"
											onclick={() => decide(i, 'disqualify')}
											disabled={c.deciding}
										>Disqualify</button>

										<button
											class="btn-decide btn-perm"
											onclick={() => decide(i, 'permanent_excuse')}
											disabled={c.deciding}
										>Perm Excuse</button>

										<button
											class="btn-decide btn-temp"
											onclick={() => decide(i, 'temporary_excuse')}
											disabled={c.deciding}
										>Temp Excuse</button>

										<button
											class="btn-decide btn-sendback"
											onclick={() => decide(i, 'send_back')}
											disabled={c.deciding}
										>Send Back</button>
									</div>

									{#if c.deciding}
										<p class="deciding-msg text-muted">Recording decision…</p>
									{/if}
								</div>
							</div>
						{/if}
					</div>
				{/if}
			{/each}
		</div>
	{/if}
</div>

<style>
	.page-wrap {
		max-width: 1100px;
		margin: 0 auto;
		padding: 1.5rem 2rem 4rem;
	}

	.page-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		margin-bottom: 1.5rem;
	}
	.page-header h1 { margin: 0 0 0.2rem; }
	.subhead { margin: 0; color: var(--text-muted); font-size: 0.92rem; }

	.loading-msg { margin-top: 2rem; }

	.maintenance-card, .empty-card {
		text-align: center;
		padding: 3rem;
		color: var(--text-muted);
	}
	.ok-icon { font-size: 2rem; color: var(--green); display: block; margin-bottom: 0.5rem; }
	.maint-icon { font-size: 2.5rem; margin-bottom: 0.75rem; }

	/* ── Case list ─────────────────────────────────────────── */
	.case-list {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	/* Expanded undecided case */
	.case-card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		overflow: hidden;
	}

	.case-head {
		background: var(--navy);
		padding: 0.85rem 1.25rem;
	}
	.case-title {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.case-name {
		font-size: 1.15rem;
		font-weight: 700;
		color: #fff;
	}
	.case-meta-head {
		font-size: 0.85rem;
		color: rgba(255, 255, 255, 0.7);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.case-body {
		display: grid;
		grid-template-columns: 1fr 380px;
		gap: 0;
	}
	@media (max-width: 860px) {
		.case-body { grid-template-columns: 1fr; }
	}

	.info-col {
		padding: 1.1rem 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		border-right: 1px solid var(--border);
	}
	@media (max-width: 860px) {
		.info-col { border-right: none; border-bottom: 1px solid var(--border); }
	}

	.info-section {}
	.info-label {
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--text-muted);
		margin-bottom: 0.4rem;
	}

	.info-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.88rem;
	}
	.info-table td {
		padding: 0.2rem 0.4rem 0.2rem 0;
		vertical-align: top;
	}
	.info-table td:first-child {
		color: var(--text-muted);
		white-space: nowrap;
		padding-right: 0.75rem;
		width: 80px;
	}

	.status-pill {
		display: inline-block;
		padding: 0.1rem 0.5rem;
		background: var(--gold-light, #fef9c3);
		color: var(--gold-dark, #854d0e);
		border-radius: 3px;
		font-size: 0.8rem;
		font-weight: 600;
	}

	.admin-notes .notes-text {
		font-size: 0.9rem;
		color: var(--text-muted);
		margin: 0;
		line-height: 1.5;
	}

	.case-loading { padding: 1rem 1.25rem; }

	/* Decision column */
	.decision-col {
		padding: 1.1rem 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0,0,0,0);
	}

	.notes-area {
		width: 100%;
		padding: 0.5rem 0.65rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-family: inherit;
		font-size: 0.88rem;
		resize: vertical;
		background: var(--surface);
		box-sizing: border-box;
	}
	.notes-area:focus {
		outline: none;
		border-color: var(--gold);
		box-shadow: 0 0 0 3px rgba(181, 152, 90, 0.15);
	}

	.form-error {
		color: var(--red);
		font-size: 0.85rem;
		font-weight: 600;
		margin: 0;
	}

	.decision-btns {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.5rem;
	}

	.btn-decide {
		padding: 0.55rem 0.5rem;
		font-size: 0.85rem;
		font-weight: 700;
		border-radius: var(--radius-sm);
		border: none;
		cursor: pointer;
		transition: opacity 0.15s, transform 0.1s;
	}
	.btn-decide:disabled { opacity: 0.4; cursor: not-allowed; }
	.btn-decide:not(:disabled):active { transform: translateY(1px); }

	.btn-requalify  { background: #dcfce7; color: #166534; }
	.btn-requalify:not(:disabled):hover  { background: #bbf7d0; }
	.btn-disqualify { background: #fef2f2; color: #991b1b; }
	.btn-disqualify:not(:disabled):hover { background: #fee2e2; }
	.btn-perm       { background: #f1f5f9; color: #475569; }
	.btn-perm:not(:disabled):hover       { background: #e2e8f0; }
	.btn-temp       { background: #fef9c3; color: #854d0e; }
	.btn-temp:not(:disabled):hover       { background: #fef08a; }
	.btn-sendback   { grid-column: 1 / -1; background: var(--navy); color: #fff; }
	.btn-sendback:not(:disabled):hover   { background: var(--navy-light, #1e3a5f); }

	.deciding-msg { font-size: 0.85rem; text-align: center; margin: 0; }

	/* ── Decided compact row ───────────────────────────────── */
	.case-decided {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.65rem 1.1rem;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		opacity: 0.7;
		gap: 1rem;
	}
	.decided-info {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.9rem;
	}
	.decided-name { font-weight: 600; }
	.decided-meta {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.decided-result {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-shrink: 0;
	}
	.decided-label {
		font-size: 0.82rem;
		font-weight: 700;
		text-transform: capitalize;
		padding: 0.15rem 0.6rem;
		border-radius: 3px;
	}
	.decided-label.resolved { background: #dcfce7; color: #166534; }
	.decided-label.sent-back { background: #f1f5f9; color: #475569; }
	.decided-at { font-size: 0.8rem; color: var(--text-muted); white-space: nowrap; }

	/* ── Type badge ────────────────────────────────────────── */
	.type-badge {
		display: inline-block;
		padding: 0.1rem 0.45rem;
		border-radius: 3px;
		font-size: 0.72rem;
		font-weight: 700;
		text-transform: capitalize;
	}
	.type-excuse    { background: #fef9c3; color: #854d0e; }
	.type-disqualify { background: #fef2f2; color: #991b1b; }

	.btn-sm { padding: 0.3rem 0.75rem; font-size: 0.82rem; }
</style>
