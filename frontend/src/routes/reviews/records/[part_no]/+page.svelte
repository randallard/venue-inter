<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { getReviewHistory } from '$lib/api';
	import type { ReviewHistoryEntry } from '$lib/types';

	const part_no = $derived($page.params.part_no ?? '');

	let entries = $state<ReviewHistoryEntry[]>([]);
	let count = $state(0);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			const res = await getReviewHistory(part_no);
			entries = res.entries;
			count = res.count;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load';
		} finally {
			loading = false;
		}
	}

	function formatDate(iso: string) {
		return new Date(iso).toLocaleString();
	}

	const actionLabel: Record<string, string> = {
		submitted: 'Submitted',
		sent_to_ceo: 'Sent to CEO',
		sent_back: 'Sent Back to Admin',
		requalify: 'Re-qualified',
		disqualify: 'Disqualified',
		permanent_excuse: 'Permanent Excuse',
		temporary_excuse: 'Temporary Excuse',
		completed: 'Completed',
	};

	const actionClass: Record<string, string> = {
		submitted: 'action-submitted',
		sent_to_ceo: 'action-sent',
		sent_back: 'action-sent-back',
		requalify: 'action-positive',
		disqualify: 'action-negative',
		permanent_excuse: 'action-excuse',
		temporary_excuse: 'action-excuse',
		completed: 'action-positive',
	};

	onMount(load);
</script>

<div class="container">
	<a class="back-link" href="/reviews">Reviews</a>

	<div class="page-header">
		<h1>Review History</h1>
		<p>Participant #{part_no}</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading…</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if entries.length === 0}
		<div class="card empty-card">
			<p class="text-muted">No review history found for participant #{part_no}.</p>
		</div>
	{:else}
		<p class="count-label">{count} {count === 1 ? 'entry' : 'entries'}</p>

		<div class="timeline">
			{#each entries as entry}
				<div class="timeline-item">
					<div class="timeline-marker {actionClass[entry.action] ?? ''}"></div>
					<div class="card timeline-card">
						<div class="entry-header">
							<span class="action-badge {actionClass[entry.action] ?? ''}">
								{actionLabel[entry.action] ?? entry.action}
							</span>
							<span class="entry-type">{entry.review_type}</span>
							<time class="entry-time">{formatDate(entry.acted_at)}</time>
						</div>
						{#if entry.actor_email}
							<p class="entry-actor">{entry.actor_email}</p>
						{/if}
						{#if entry.notes}
							<p class="entry-notes">{entry.notes}</p>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.count-label {
		font-size: 0.88rem;
		color: var(--text-muted);
		margin-bottom: 1rem;
	}

	.timeline {
		display: flex;
		flex-direction: column;
		gap: 0;
		position: relative;
	}

	.timeline::before {
		content: '';
		position: absolute;
		left: 0.65rem;
		top: 0.75rem;
		bottom: 0.75rem;
		width: 2px;
		background: var(--border);
	}

	.timeline-item {
		display: flex;
		gap: 1rem;
		align-items: flex-start;
		padding-bottom: 1rem;
	}

	.timeline-marker {
		flex-shrink: 0;
		width: 1.35rem;
		height: 1.35rem;
		border-radius: 50%;
		margin-top: 0.6rem;
		background: var(--border);
		border: 2px solid var(--surface);
		z-index: 1;
	}

	.timeline-marker.action-positive { background: var(--green, #16a34a); }
	.timeline-marker.action-negative { background: var(--red, #dc2626); }
	.timeline-marker.action-sent     { background: var(--gold, #b5985a); }
	.timeline-marker.action-sent-back { background: var(--text-muted, #6b7280); }
	.timeline-marker.action-excuse   { background: #7c3aed; }
	.timeline-marker.action-submitted { background: #2563eb; }

	.timeline-card {
		flex: 1;
		padding: 0.75rem 1rem;
	}

	.entry-header {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		flex-wrap: wrap;
	}

	.action-badge {
		font-size: 0.82rem;
		font-weight: 600;
		padding: 0.15rem 0.5rem;
		border-radius: 3px;
		background: var(--surface-alt, #f3f4f6);
		color: var(--text);
	}

	.action-badge.action-positive { background: #dcfce7; color: #15803d; }
	.action-badge.action-negative { background: #fee2e2; color: #b91c1c; }
	.action-badge.action-sent     { background: #fef9c3; color: #92400e; }
	.action-badge.action-sent-back { background: #f3f4f6; color: #374151; }
	.action-badge.action-excuse   { background: #ede9fe; color: #6d28d9; }
	.action-badge.action-submitted { background: #dbeafe; color: #1d4ed8; }

	.entry-type {
		font-size: 0.82rem;
		color: var(--text-muted);
		text-transform: capitalize;
	}

	.entry-time {
		font-size: 0.82rem;
		color: var(--text-muted);
		margin-left: auto;
	}

	.entry-actor {
		margin: 0.35rem 0 0;
		font-size: 0.85rem;
		color: var(--text-muted);
	}

	.entry-notes {
		margin: 0.35rem 0 0;
		font-size: 0.9rem;
		color: var(--text);
		font-style: italic;
	}

	.empty-card {
		padding: 2rem;
		text-align: center;
	}
</style>
