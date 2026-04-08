<script lang="ts">
	import '../app.css';
	import NavBar from '$lib/components/NavBar.svelte';
	import Toast from '$lib/components/Toast.svelte';
	import type { LayoutData } from './$types';

	let { data, children }: { data: LayoutData; children: any } = $props();
</script>

<NavBar user={data.user} />
{@render children()}
<Toast />
