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
			<a class="nav-link" class:active={isActive('/reviews/queue')} href="/reviews/queue">Review Queue</a>
		</div>
	{:else}
		<div class="nav-links">
			<a class="nav-link" class:active={isActive('/')} href="/">Dashboard</a>
			<a class="nav-link" class:active={isActive('/pools')} href="/pools">Pools</a>
			<a class="nav-link" class:active={isActive('/reviews')} href="/reviews">Reviews</a>
			<a class="nav-link" class:active={isActive('/reports')} href="/reports">Reports</a>
			<a class="nav-link" class:active={isActive('/data')} href="/data">Data</a>
			<a class="nav-link" class:active={isActive('/tasks')} href="/tasks">Tasks</a>
		</div>
	{/if}
	<div class="nav-right">
		{#if user}
			<span>{display}</span>
			<form method="POST" action="/auth/logout" style="display:contents">
				<button type="submit">Logout</button>
			</form>
		{:else}
			<a href="/auth/login">Login</a>
		{/if}
	</div>
</nav>
