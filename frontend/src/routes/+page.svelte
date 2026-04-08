<script lang="ts">
	import { onMount } from 'svelte';
	import { getDashboardStatus } from '$lib/api';
	import type { DashboardStatus } from '$lib/types';

	let status = $state<DashboardStatus | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);

	async function load() {
		loading = true;
		error = null;
		try {
			status = await getDashboardStatus();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load dashboard';
		} finally {
			loading = false;
		}
	}

	onMount(load);
</script>

<div class="container">
	<div class="page-header">
		<h1>Dashboard</h1>
		<p>Venue audience management overview</p>
	</div>

	{#if loading}
		<div class="status-grid">
			{#each Array(5) as _}
				<div class="status-card loading-card">
					<div class="skeleton-line short"></div>
					<div class="skeleton-line count"></div>
				</div>
			{/each}
		</div>
	{:else if error}
		<p class="error">{error}</p>
	{:else if status}
		<div class="status-grid">
			<a class="status-card {status.bad_show_codes > 0 ? 'warn' : 'ok'}" href="/pools/fix-show-codes">
				<div class="card-label">Bad Show Codes</div>
				<div class="card-count">{status.bad_show_codes}</div>
				<div class="card-sub">{status.bad_show_codes === 0 ? 'All codes valid' : 'pools need correction'}</div>
			</a>

			<a class="status-card {status.blank_questionnaires > 0 ? 'warn' : 'ok'}" href="/pools/reset-questionnaire">
				<div class="card-label">Blank Questionnaires</div>
				<div class="card-count">{status.blank_questionnaires}</div>
				<div class="card-sub">{status.blank_questionnaires === 0 ? 'All submitted' : 'members with blank QQ'}</div>
			</a>

			<a class="status-card {status.portal_lockouts > 0 ? 'warn' : 'ok'}" href="/pools/lockouts">
				<div class="card-label">Portal Lockouts</div>
				<div class="card-count">{status.portal_lockouts}</div>
				<div class="card-sub">{status.portal_lockouts === 0 ? 'No lockouts' : 'participants locked out'}</div>
			</a>

			<div class="status-card {status.informix_sync_pending === 0 ? 'ok' : 'warn'}">
				<div class="card-label">Sync Pending</div>
				<div class="card-count">{status.informix_sync_pending}</div>
				<div class="card-sub">{status.informix_sync_pending === 0 ? 'Queue clear' : 'records awaiting sync'}</div>
			</div>

			<div class="status-card {status.informix_sync_failed === 0 ? 'ok' : 'crit'}">
				<div class="card-label">Sync Failed</div>
				<div class="card-count">{status.informix_sync_failed}</div>
				<div class="card-sub">{status.informix_sync_failed === 0 ? 'No failures' : 'records need attention'}</div>
			</div>
		</div>
	{/if}

	<div class="refresh-row">
		<button class="btn btn-secondary" onclick={load} disabled={loading}>
			{loading ? 'Refreshing…' : 'Refresh'}
		</button>
	</div>
</div>

<style>
	.status-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: 1rem;
		margin-bottom: 1.5rem;
	}

	.status-card {
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		padding: 1.25rem 1.5rem;
		box-shadow: var(--shadow);
		text-decoration: none;
		color: var(--text);
		transition: box-shadow 0.15s, border-color 0.15s;
		border-left: 4px solid var(--border);
	}

	a.status-card:hover {
		box-shadow: var(--shadow-lg);
		text-decoration: none;
		border-left-color: var(--gold);
	}

	.status-card.ok   { border-left-color: var(--green); }
	.status-card.warn { border-left-color: var(--amber); }
	.status-card.crit { border-left-color: var(--red); }

	.card-label {
		font-size: 0.82rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-muted);
		margin-bottom: 0.4rem;
	}

	.card-count {
		font-size: 2.2rem;
		font-weight: 700;
		color: var(--navy);
		line-height: 1;
		margin-bottom: 0.35rem;
	}

	.status-card.warn .card-count { color: var(--amber); }
	.status-card.crit .card-count { color: var(--red); }

	.card-sub {
		font-size: 0.82rem;
		color: var(--text-muted);
	}

	.loading-card {
		min-height: 110px;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 0.6rem;
	}

	.skeleton-line {
		height: 0.8rem;
		background: linear-gradient(90deg, var(--border) 25%, #f0f0f0 50%, var(--border) 75%);
		background-size: 200% 100%;
		border-radius: 3px;
		animation: shimmer 1.5s infinite;
	}
	.skeleton-line.short { width: 55%; }
	.skeleton-line.count { width: 30%; height: 2rem; }

	@keyframes shimmer {
		0%   { background-position: 200% 0; }
		100% { background-position: -200% 0; }
	}

	.refresh-row {
		display: flex;
		justify-content: flex-end;
	}
</style>
