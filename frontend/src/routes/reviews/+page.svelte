<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getPendingCounts } from '$lib/api';
	import type { PendingCountsResponse } from '$lib/types';

	let counts = $state<PendingCountsResponse | null>(null);
	let historyPartNo = $state('');

	function lookupHistory() {
		const trimmed = historyPartNo.trim();
		if (trimmed) goto(`/reviews/records/${trimmed}`);
	}

	function onHistoryKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') lookupHistory();
	}

	onMount(async () => {
		try {
			counts = await getPendingCounts();
		} catch {
			// non-fatal — counts just won't show
		}
	});
</script>

<div class="container">
	<div class="page-header">
		<h1>Reviews</h1>
		<p>Excuse and disqualification review workflow</p>
	</div>

	<div class="queue-grid">
		<a class="queue-card" href="/reviews/excuse">
			<div class="queue-label">Excuse Requests</div>
			{#if counts !== null}
				<div class="queue-count {counts.excuse_pending > 0 ? 'has-items' : ''}">
					{counts.excuse_pending}
				</div>
				<div class="queue-sub">{counts.excuse_pending === 0 ? 'No pending requests' : 'pending admin review'}</div>
			{:else}
				<div class="queue-count">—</div>
				<div class="queue-sub">Admin review queue</div>
			{/if}
		</a>

		<a class="queue-card" href="/reviews/disqualify">
			<div class="queue-label">Disqualification Requests</div>
			{#if counts !== null}
				<div class="queue-count {counts.disqualify_pending > 0 ? 'has-items' : ''}">
					{counts.disqualify_pending}
				</div>
				<div class="queue-sub">{counts.disqualify_pending === 0 ? 'No pending requests' : 'pending admin review'}</div>
			{:else}
				<div class="queue-count">—</div>
				<div class="queue-sub">Admin review queue</div>
			{/if}
		</a>

		<a class="queue-card queue-card-ceo" href="/reviews/ceo">
			<div class="queue-label">CEO Queue</div>
			{#if counts !== null}
				<div class="queue-count {counts.ceo_queue > 0 ? 'has-items' : ''}">
					{counts.ceo_queue}
				</div>
				<div class="queue-sub">{counts.ceo_queue === 0 ? 'No cases pending' : 'cases awaiting decision'}</div>
			{:else}
				<div class="queue-count">—</div>
				<div class="queue-sub">Cases sent to CEO</div>
			{/if}
		</a>
	</div>

	<div class="history-section">
		<h2>Review History Lookup</h2>
		<p class="text-muted">Enter a participant number to view their full review history.</p>
		<div class="history-input-row">
			<input
				class="history-input"
				type="text"
				placeholder="Participant #"
				bind:value={historyPartNo}
				onkeydown={onHistoryKeydown}
			/>
			<button class="btn btn-secondary" onclick={lookupHistory} disabled={!historyPartNo.trim()}>
				View History
			</button>
		</div>
	</div>
</div>

<style>
	.queue-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 1rem;
	}

	.queue-card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 1.5rem;
		box-shadow: var(--shadow);
		text-decoration: none;
		color: var(--text);
		border-left: 4px solid var(--border);
		transition: box-shadow 0.15s, border-left-color 0.15s;
	}
	.queue-card:hover {
		box-shadow: var(--shadow-lg);
		border-left-color: var(--gold);
		text-decoration: none;
	}
	.queue-card-ceo { border-left-color: var(--navy-light); }

	.queue-label {
		font-size: 0.82rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-muted);
		margin-bottom: 0.4rem;
	}

	.queue-count {
		font-size: 2.4rem;
		font-weight: 700;
		color: var(--navy);
		line-height: 1;
		margin-bottom: 0.35rem;
	}
	.queue-count.has-items { color: var(--amber); }

	.queue-sub {
		font-size: 0.82rem;
		color: var(--text-muted);
	}

	.history-section {
		margin-top: 2rem;
		padding-top: 1.5rem;
		border-top: 1px solid var(--border);
	}

	.history-section h2 {
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: 0.3rem;
	}

	.history-section .text-muted {
		margin-bottom: 0.75rem;
	}

	.history-input-row {
		display: flex;
		gap: 0.5rem;
		max-width: 360px;
	}

	.history-input {
		flex: 1;
		padding: 0.45rem 0.75rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		font-size: 0.9rem;
		background: var(--surface);
		color: var(--text);
	}

	.history-input:focus {
		outline: none;
		border-color: var(--gold);
		box-shadow: 0 0 0 3px rgba(181,152,90,0.15);
	}
</style>
