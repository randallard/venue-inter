import { redirect } from '@sveltejs/kit';
import { getCurrentUser } from '$lib/api';
import type { UserSession } from '$lib/types';

export const ssr = false;

export async function load({ url }: { url: URL }): Promise<{ user: UserSession | null }> {
	let user: UserSession | null = null;
	try {
		user = await getCurrentUser();
	} catch {
		// not authenticated — let the page handle it
	}
	// CEO users land directly on the review queue, not the admin dashboard
	if (user?.groups.includes('ceo-review') && url.pathname === '/') {
		redirect(302, '/reviews/ceo');
	}
	return { user };
}
