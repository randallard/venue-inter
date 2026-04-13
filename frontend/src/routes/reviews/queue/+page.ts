import type { LayoutData } from '../$types';

export function load({ parent, url }: { parent: () => Promise<LayoutData>; url: URL }) {
	return parent().then((data) => ({
		user: data.user,
		initialType: url.searchParams.get('type') ?? 'all',
		initialStatus: url.searchParams.get('status') ?? null
	}));
}
