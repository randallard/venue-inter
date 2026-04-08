<script lang="ts">
	import { page } from '$app/state';
	import type { UserSession } from '$lib/types';

	let { user }: { user: UserSession | null } = $props();

	const isCeo = $derived(user?.groups.includes('ceo-review') ?? false);

	const display = $derived(
		user ? (user.name ?? user.email ?? user.sub) : null
	);

	function isActive(href: string): boolean {
		if (href === '/') return page.url.pathname === '/';
		return page.url.pathname.startsWith(href);
	}
</script>

<nav class="navbar">
	<a class="navbar-brand" href={isCeo ? '/reviews/ceo' : '/'}>
		VenueInter
	</a>
	{#if isCeo}
		<div class="nav-links">
			<a class="nav-link" class:active={isActive('/reviews/ceo')} href="/reviews/ceo">Review Queue</a>
		</div>
	{:else}
		<div class="nav-links">
			<a class="nav-link" class:active={isActive('/')} href="/">Dashboard</a>
			<a class="nav-link" class:active={isActive('/pools')} href="/pools">Pools</a>
			<a class="nav-link" class:active={isActive('/reviews')} href="/reviews">Reviews</a>
			<a class="nav-link" class:active={isActive('/reports')} href="/reports">Reports</a>
			<a class="nav-link" class:active={isActive('/data')} href="/data">Data</a>
		</div>
	{/if}
	<div class="nav-right">
		{#if user}
			<span>{display}</span>
			<a href="/auth/logout">Logout</a>
		{:else}
			<a href="/auth/login">Login</a>
		{/if}
	</div>
</nav>
