import { getCurrentUser } from '$lib/api';
import type { UserSession } from '$lib/types';

export const ssr = false;

export async function load(): Promise<{ user: UserSession | null }> {
	try {
		const user = await getCurrentUser();
		return { user };
	} catch {
		return { user: null };
	}
}
