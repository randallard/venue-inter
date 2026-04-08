<script lang="ts">
	import { page } from '$app/state';
	import { getMasterList } from '$lib/api';
	import type { MasterResponse } from '$lib/types';

	let data = $state<MasterResponse | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let currentPage = $state(0);

	const slug = $derived(page.params.slug!);

	async function load(p: number) {
		loading = true;
		error = null;
		try {
			data = await getMasterList(slug, p);
			currentPage = p;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load data';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		load(0);
	});

	const totalPages = $derived(
		data ? Math.ceil(data.total_count / Math.max(data.page_size, 1)) : 0
	);
	const hasPrev = $derived(currentPage > 0);
	const hasNext = $derived(currentPage + 1 < totalPages);
</script>

<div class="container">
	{#if data}
		<div class="page-header">
			<h1>{data.link_name}</h1>
			<p class="count-label">
				{data.total_count} total records — Page {data.page + 1} of {totalPages}
			</p>
		</div>
	{:else}
		<div class="page-header">
			<h1>Loading...</h1>
		</div>
	{/if}

	{#if error}
		<p class="error">{error}</p>
	{:else if loading && !data}
		<div class="table-wrap">
			<table>
				<tbody>
					{#each Array(5) as _}
						<tr class="skeleton-row">
							<td>&nbsp;</td>
							<td>&nbsp;</td>
							<td>&nbsp;</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{:else if data}
		<div class="table-wrap" class:table-loading={loading}>
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
							{#each row as val, i}
								{@const col = data.columns[i]}
								<td>
									{#if col?.link_to_detail}
										<a href="/data/{slug}/{val}">{val}</a>
									{:else}
										{val}
									{/if}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>

		<div class="pagination">
			<button class="btn btn-secondary" disabled={!hasPrev} onclick={() => load(currentPage - 1)}>
				Prev
			</button>
			<span class="page-info">Page {currentPage + 1} of {totalPages}</span>
			<button class="btn btn-secondary" disabled={!hasNext} onclick={() => load(currentPage + 1)}>
				Next
			</button>
		</div>
	{/if}
</div>
