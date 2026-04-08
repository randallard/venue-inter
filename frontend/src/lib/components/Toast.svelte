<script lang="ts">
	import { getToasts } from '$lib/toast.svelte';

	const toasts = $derived(getToasts());
</script>

{#if toasts.length > 0}
	<div class="toast-container">
		{#each toasts as toast (toast.id)}
			<div class="toast toast-{toast.type}">{toast.message}</div>
		{/each}
	</div>
{/if}
