<script lang="ts">
	import { page } from '$app/state';
	import { getDetail } from '$lib/api';
	import type { DetailResponse } from '$lib/types';

	let data = $state<DetailResponse | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	const slug = $derived(page.params.slug!);
	const id = $derived(page.params.id!);

	async function load() {
		loading = true;
		error = null;
		try {
			data = await getDetail(slug, id);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load detail';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load();
	});
</script>

<div class="container">
	<a class="back-link" href="/data/{slug}">
		{data?.link_name ?? 'Back'}
	</a>

	{#if data}
		<div class="page-header">
			<h1>Detail: {data.id_value}</h1>
			<p class="count-label">{data.rows.length} record(s)</p>
		</div>
	{/if}

	{#if error}
		<p class="error">{error}</p>
	{:else if loading}
		<div class="table-wrap">
			<table>
				<tbody>
					{#each Array(3) as _}
						<tr class="skeleton-row">
							<td>&nbsp;</td>
							<td>&nbsp;</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else if data}
		{#if data.rows.length === 0}
			<p class="text-muted">No detail records found.</p>
		{:else}
			<div class="table-wrap">
				<table>
					<thead>
						<tr>
							{#each data.columns as col}
								<th>{col.label}</th>
							{/each}
						</tr>
					</thead>
					<tbody>
						{#each data.rows as row}
							<tr>
								{#each row as val}
									<td>{val}</td>
								{/each}
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	{/if}
</div>
