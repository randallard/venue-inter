<script lang="ts">
	import { getQueryLinks } from '$lib/api';
	import type { QueryLink } from '$lib/types';

	let links = $state<QueryLink[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	async function load() {
		try {
			links = await getQueryLinks();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load queries';
		} finally {
			loading = false;
		}
	}

	load();
</script>

<div class="container">
	<div class="page-header">
		<h1>Data</h1>
		<p>Query and explore venue data</p>
	</div>

	{#if loading}
		<p class="text-muted">Loading queries...</p>
	{:else if error}
		<p class="error">{error}</p>
	{:else if links.length === 0}
		<p class="text-muted">No queries configured.</p>
	{:else}
		<div class="card">
			<ul class="feature-list">
				{#each links as link}
					<li>
						<a href="/data/{link.slug}">{link.name}</a>
					</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>
