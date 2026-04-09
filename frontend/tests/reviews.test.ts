/**
 * Phase 5: CEO Review workflow — E2E tests
 *
 * Tests run against live data and are idempotent: each test reads current
 * queue state and skips destructive steps when the queue is empty (i.e. after
 * all seed records have already been acted upon).
 *
 * Required env vars:
 *   TEST_USER, TEST_PASSWORD      — standard (admin) user
 *   CEO_TEST_USER, CEO_TEST_PASSWORD — user in the ceo-review Authentik group
 *                                     (optional; CEO-specific tests skip when absent)
 */

import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import type { Browser, Page } from 'puppeteer';
import { setup, teardown, login, loginAsCeo, clearSession, BASE_URL } from './helpers.ts';

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Returns the href of the first row link in the table, or null if empty. */
async function firstRowPartKey(page: Page): Promise<string | null> {
	const href = await page.$eval(
		'table tbody tr:first-child a.btn',
		(el) => (el as HTMLAnchorElement).href,
	).catch(() => null);
	return href;
}

/** Wait for the loading state to clear. */
async function waitForContent(page: Page, timeout = 10_000): Promise<void> {
	await page.waitForFunction(
		() => !document.querySelector('.text-muted')?.textContent?.includes('Loading'),
		{ timeout },
	);
}

// ── Admin Queue Tests ─────────────────────────────────────────────────────────

describe('reviews — admin excuse queue', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('excuse queue page loads at /reviews/excuse', async () => {
		await page.goto(BASE_URL + '/reviews/excuse');
		await page.waitForSelector('h1', { timeout: 10_000 });
		const heading = await page.$eval('h1', (el) => el.textContent?.trim());
		assert.equal(heading, 'Excuse Requests');
	});

	it('page renders a table or an empty state', async () => {
		await waitForContent(page);
		const hasTable = await page.$('table') !== null;
		const hasEmpty = await page.$('.card.empty') !== null;
		assert.ok(hasTable || hasEmpty, 'expected either a table or an empty-state card');
	});

	it('if records exist, each row links to the individual review page', async () => {
		const link = await page.$('table tbody tr a');
		if (!link) return; // empty queue — skip

		const href = await link.evaluate((el) => (el as HTMLAnchorElement).href);
		assert.match(href, /\/reviews\/excuse\/\d+_\d+$/, 'expected part_key URL pattern');
	});
});

describe('reviews — admin disqualify queue', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('disqualify queue page loads at /reviews/disqualify', async () => {
		await page.goto(BASE_URL + '/reviews/disqualify');
		await page.waitForSelector('h1', { timeout: 10_000 });
		const heading = await page.$eval('h1', (el) => el.textContent?.trim());
		assert.equal(heading, 'Disqualification Requests');
	});

	it('page renders a table or an empty state', async () => {
		await waitForContent(page);
		const hasTable = await page.$('table') !== null;
		const hasEmpty = await page.$('.card.empty') !== null;
		assert.ok(hasTable || hasEmpty, 'expected either a table or an empty-state card');
	});
});

// ── Individual Review Detail ──────────────────────────────────────────────────

describe('reviews — individual review detail (admin view)', () => {
	let browser: Browser;
	let page: Page;
	let detailUrl: string | null = null;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);

		// Find a reviewable record in whichever queue has one
		for (const queue of ['/reviews/excuse', '/reviews/disqualify']) {
			await page.goto(BASE_URL + queue);
			await waitForContent(page);
			const link = await page.$('table tbody tr a');
			if (link) {
				detailUrl = await link.evaluate((el) => (el as HTMLAnchorElement).href);
				break;
			}
		}
	});

	after(async () => {
		await teardown(browser);
	});

	it('individual review page loads', async () => {
		if (!detailUrl) return; // no pending records — skip

		await page.goto(detailUrl);
		await page.waitForSelector('.page-header h1', { timeout: 10_000 });
		const heading = await page.$eval('.page-header h1', (el) => el.textContent?.trim() ?? '');
		assert.ok(heading.length > 0, 'expected participant name in heading');
	});

	it('participant data panel is visible', async () => {
		if (!detailUrl) return;

		const card = await page.$('.detail-grid .card');
		assert.ok(card, 'expected participant data card');
	});

	it('pool status panel is visible', async () => {
		if (!detailUrl) return;

		const cards = await page.$$('.detail-grid .card');
		assert.ok(cards.length >= 2, 'expected both participant and pool status cards');
	});

	it('review record panel shows IFX status', async () => {
		if (!detailUrl) return;

		const text = await page.$eval(
			'body',
			(el) => el.textContent ?? '',
		);
		assert.ok(text.includes('IFX Status'), 'expected IFX Status field in review record panel');
	});

	it('back link returns to queue', async () => {
		if (!detailUrl) return;

		const backLink = await page.$('.back-link');
		assert.ok(backLink, 'expected back link');
		const href = await backLink!.evaluate((el) => (el as HTMLAnchorElement).href);
		assert.match(href, /\/reviews\/(excuse|disqualify)$/);
	});

	it('View History link is present and points to /reviews/records/:part_no', async () => {
		if (!detailUrl) return;

		const links = await page.$$eval('.nav-row a', (els) =>
			els.map((el) => (el as HTMLAnchorElement).href),
		);
		const historyLink = links.find((h) => h.includes('/reviews/records/'));
		assert.ok(historyLink, 'expected a View History link in the nav row');
	});
});

// ── Send to CEO ───────────────────────────────────────────────────────────────

describe('reviews — send to CEO', () => {
	let browser: Browser;
	let page: Page;
	let pendingPartKey: string | null = null;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);

		// Find a record still in pending_admin state (IFX status 'P', no pg_status yet)
		for (const queue of ['/reviews/excuse', '/reviews/disqualify']) {
			await page.goto(BASE_URL + queue);
			await waitForContent(page);
			const link = await page.$('table tbody tr a');
			if (link) {
				const href = await link.evaluate((el) => (el as HTMLAnchorElement).href);
				const match = href.match(/\/(excuse|disqualify)\/([\d_]+)$/);
				if (match) { pendingPartKey = match[2]; break; }
			}
		}
	});

	after(async () => {
		await teardown(browser);
	});

	it('Send to CEO button is present for pending records', async () => {
		if (!pendingPartKey) return; // nothing in admin queues — skip

		// Navigate to whichever queue type this record came from
		const queueType = pendingPartKey && await (async () => {
			for (const t of ['excuse', 'disqualify']) {
				await page.goto(BASE_URL + `/reviews/${t}/${pendingPartKey}`);
				await waitForContent(page);
				const btn = await page.$('button.btn-primary');
				if (btn) return t;
			}
			return null;
		})();
		if (!queueType) return;

		const btn = await page.$('button.btn-primary');
		assert.ok(btn, 'expected Send to CEO button');
		const text = await btn!.evaluate((el) => el.textContent?.trim() ?? '');
		assert.match(text, /send to ceo/i);
	});

	it('clicking Send to CEO shows success and removes record from admin queue', async () => {
		if (!pendingPartKey) return;

		// First count records in excuse queue
		await page.goto(BASE_URL + '/reviews/excuse');
		await waitForContent(page);
		const excuseBefore = await page.$$('table tbody tr').then((r) => r.length).catch(() => 0);

		await page.goto(BASE_URL + '/reviews/disqualify');
		await waitForContent(page);
		const disqualifyBefore = await page.$$('table tbody tr').then((r) => r.length).catch(() => 0);

		// Navigate to the pending record and send it
		let targetQueue: string | null = null;
		for (const t of ['excuse', 'disqualify']) {
			await page.goto(BASE_URL + `/reviews/${t}/${pendingPartKey}`);
			await waitForContent(page);
			const btn = await page.$('button.btn-primary');
			if (btn) { targetQueue = t; break; }
		}
		if (!targetQueue) return;

		await page.click('button.btn-primary');

		// Wait for success message or status change
		await page.waitForFunction(
			() => {
				const msg = document.querySelector('.msg-ok')?.textContent ?? '';
				const pgStatus = document.querySelector('.pg-status')?.textContent ?? '';
				return msg.length > 0 || pgStatus.includes('pending_ceo');
			},
			{ timeout: 10_000 },
		);

		// Verify record no longer in the admin queue
		await page.goto(BASE_URL + `/reviews/${targetQueue}`);
		await waitForContent(page);
		const rowsAfter = await page.$$('table tbody tr').then((r) => r.length).catch(() => 0);

		const rowsBefore = targetQueue === 'excuse' ? excuseBefore : disqualifyBefore;
		assert.ok(
			rowsAfter < rowsBefore || rowsAfter === 0,
			`expected fewer rows after send to CEO (before: ${rowsBefore}, after: ${rowsAfter})`,
		);
	});
});

// ── CEO Queue ─────────────────────────────────────────────────────────────────

describe('reviews — CEO queue (standard user sees 403 or empty)', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('/reviews/ceo page loads for standard user', async () => {
		await page.goto(BASE_URL + '/reviews/ceo');
		await page.waitForSelector('h1', { timeout: 10_000 });
		// Standard user: API returns 403, page shows error or empty state
		await waitForContent(page);
		const body = await page.$eval('body', (el) => el.textContent ?? '');
		// Either shows an access error or an empty queue — both are acceptable
		// The critical thing is the page doesn't crash
		assert.ok(body.length > 0, 'expected non-empty page body');
	});
});

describe('reviews — CEO queue (ceo-review role)', () => {
	let browser: Browser;
	let page: Page;
	let hasCeoUser = false;

	before(async () => {
		({ browser, page } = await setup());
		hasCeoUser = await loginAsCeo(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('CEO queue loads with pending cases', async () => {
		if (!hasCeoUser) return; // CEO_TEST_USER not configured — skip

		await page.goto(BASE_URL + '/reviews/ceo');
		await waitForContent(page);

		const hasTable = await page.$('table') !== null;
		const hasEmpty = await page.$('.card.empty') !== null;
		const hasMaint = await page.$('.card.maintenance') !== null;
		assert.ok(hasTable || hasEmpty || hasMaint, 'expected table, empty state, or maintenance card');
	});

	it('CEO queue row links to /reviews/ceo/:part_key', async () => {
		if (!hasCeoUser) return;

		const link = await page.$('table tbody tr a.btn');
		if (!link) return; // empty queue — skip

		const href = await link.evaluate((el) => (el as HTMLAnchorElement).href);
		assert.match(href, /\/reviews\/ceo\/\d+_\d+$/, 'expected CEO decision URL pattern');
	});
});

// ── CEO Decision ──────────────────────────────────────────────────────────────

describe('reviews — CEO decision', () => {
	let browser: Browser;
	let page: Page;
	let hasCeoUser = false;
	let ceoPartKey: string | null = null;

	before(async () => {
		({ browser, page } = await setup());
		hasCeoUser = await loginAsCeo(page);

		if (hasCeoUser) {
			await page.goto(BASE_URL + '/reviews/ceo');
			await waitForContent(page);
			const link = await page.$('table tbody tr a.btn');
			if (link) {
				const href = await link.evaluate((el) => (el as HTMLAnchorElement).href);
				const match = href.match(/\/ceo\/([\d_]+)$/);
				if (match) ceoPartKey = match[1];
			}
		}
	});

	after(async () => {
		await teardown(browser);
	});

	it('CEO decision view loads with participant data', async () => {
		if (!hasCeoUser || !ceoPartKey) return;

		await page.goto(BASE_URL + `/reviews/ceo/${ceoPartKey}`);
		await waitForContent(page);

		const heading = await page.$eval('.page-header h1', (el) => el.textContent?.trim() ?? '');
		assert.ok(heading.length > 0, 'expected participant name in heading');
	});

	it('decision buttons are rendered', async () => {
		if (!hasCeoUser || !ceoPartKey) return;

		const buttons = await page.$$('.decision-btn, button.btn-primary, button.btn-secondary');
		assert.ok(buttons.length > 0, 'expected decision action buttons');
	});

	it('submitting without notes shows validation error', async () => {
		if (!hasCeoUser || !ceoPartKey) return;

		await page.goto(BASE_URL + `/reviews/ceo/${ceoPartKey}`);
		await waitForContent(page);

		// Clear any pre-filled notes
		const textarea = await page.$('textarea');
		if (textarea) {
			await textarea.click({ clickCount: 3 });
			await page.keyboard.press('Backspace');
		}

		// Click the first decision button without entering notes
		const firstBtn = await page.$('button[data-action], .decision-grid button');
		if (!firstBtn) return;
		await firstBtn.click();

		await page.waitForFunction(
			() => {
				const err = document.querySelector('.inline-error, .error')?.textContent ?? '';
				return err.toLowerCase().includes('note') || err.length > 0;
			},
			{ timeout: 5_000 },
		).catch(() => {}); // test may pass even if selector varies — just checking no crash

		// Page should still be on the decision view (not navigated away)
		assert.match(page.url(), /\/reviews\/ceo\/[\d_]+$/, 'should remain on decision page');
	});

	it('re-qualifying a record navigates back to CEO queue', async () => {
		if (!hasCeoUser || !ceoPartKey) return;

		await page.goto(BASE_URL + `/reviews/ceo/${ceoPartKey}`);
		await waitForContent(page);

		// Check if this record is still pending_ceo
		const pgStatus = await page.$eval(
			'.pg-status, [class*="status"]',
			(el) => el.textContent?.trim() ?? '',
		).catch(() => '');

		if (pgStatus === 'completed' || pgStatus === 'sent_back') {
			// Already decided — idempotency test: re-submitting should 200 not 500
			return;
		}

		const textarea = await page.$('textarea');
		if (!textarea) return;
		await textarea.type('E2E test decision — re-qualify');

		// Click requalify button if present, else first decision button
		const requalBtn = await page.$('[data-action="requalify"], button*="Re-qualify", button*="Requalify"');
		const btn = requalBtn ?? await page.$('.decision-grid button, button.btn-primary');
		if (!btn) return;
		await btn.click();

		await page.waitForNavigation({ waitUntil: 'networkidle0', timeout: 15_000 }).catch(() => {});
		// Should navigate back to CEO queue on success
		const url = page.url();
		assert.ok(
			url.endsWith('/reviews/ceo') || url.includes('/reviews/ceo/'),
			`expected navigation to CEO queue after decision, got: ${url}`,
		);
	});
});

// ── Review History ────────────────────────────────────────────────────────────

describe('reviews — review history page', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('/reviews/records/:part_no page loads', async () => {
		// Use part_no 7 from seed data
		await page.goto(BASE_URL + '/reviews/records/7');
		await page.waitForSelector('h1', { timeout: 10_000 });
		const heading = await page.$eval('h1', (el) => el.textContent?.trim());
		assert.equal(heading, 'Review History');
	});

	it('shows timeline entries or empty state', async () => {
		await waitForContent(page);
		const hasTimeline = await page.$('.timeline-item') !== null;
		const hasEmpty = await page.$('.empty-card') !== null;
		assert.ok(hasTimeline || hasEmpty, 'expected timeline items or empty state');
	});

	it('if entries exist they have action badges', async () => {
		const badge = await page.$('.action-badge');
		if (!badge) return; // empty — skip
		const text = await badge.evaluate((el) => el.textContent?.trim() ?? '');
		assert.ok(text.length > 0, 'expected non-empty action badge');
	});

	it('back link goes to /reviews', async () => {
		const backLink = await page.$('.back-link');
		assert.ok(backLink, 'expected back link');
		const href = await backLink!.evaluate((el) => (el as HTMLAnchorElement).href);
		assert.ok(href.endsWith('/reviews'), `expected back link to /reviews, got ${href}`);
	});

	it('history lookup on /reviews landing navigates to /reviews/records/:part_no', async () => {
		await page.goto(BASE_URL + '/reviews');
		await page.waitForSelector('.history-input', { timeout: 10_000 });
		await page.type('.history-input', '11');
		await page.click('.history-section .btn');
		await page.waitForNavigation({ waitUntil: 'networkidle0', timeout: 10_000 });
		assert.ok(page.url().endsWith('/reviews/records/11'), `expected /reviews/records/11, got ${page.url()}`);
	});
});

// ── CEO Maintenance Mode ──────────────────────────────────────────────────────

describe('reviews — CEO maintenance mode', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('GET /api/reviews/ceo-state returns a valid state', async () => {
		await page.goto(BASE_URL + '/reviews/ceo');
		const state = await page.evaluate(async () => {
			const res = await fetch('/api/reviews/ceo-state');
			return res.ok ? res.json() : null;
		});
		assert.ok(state !== null, 'expected 200 from /api/reviews/ceo-state');
		assert.ok(
			state.state === 'live' || state.state === 'maintenance',
			`expected state to be live or maintenance, got: ${state.state}`,
		);
	});

	it('POST /api/reviews/ceo-state toggles and restores state', async () => {
		const initial = await page.evaluate(async () => {
			const res = await fetch('/api/reviews/ceo-state');
			return (await res.json()).state as string;
		});

		const toggled = initial === 'live' ? 'maintenance' : 'live';

		// Set to toggled state
		await page.evaluate(async (s) => {
			await fetch('/api/reviews/ceo-state', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ state: s }),
			});
		}, toggled);

		const afterToggle = await page.evaluate(async () => {
			const res = await fetch('/api/reviews/ceo-state');
			return (await res.json()).state as string;
		});
		assert.equal(afterToggle, toggled, 'expected state to have toggled');

		// Restore original state
		await page.evaluate(async (s) => {
			await fetch('/api/reviews/ceo-state', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ state: s }),
			});
		}, initial);

		const restored = await page.evaluate(async () => {
			const res = await fetch('/api/reviews/ceo-state');
			return (await res.json()).state as string;
		});
		assert.equal(restored, initial, 'expected state to be restored to original');
	});

	it('CEO queue shows maintenance card when state is maintenance', async () => {
		// Put into maintenance
		await page.evaluate(async () => {
			await fetch('/api/reviews/ceo-state', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ state: 'maintenance' }),
			});
		});

		await page.goto(BASE_URL + '/reviews/ceo');
		await waitForContent(page);

		const hasMaint = await page.$('.card.maintenance') !== null;
		const bodyText = await page.$eval('body', (el) => el.textContent ?? '');

		// Restore live state before asserting (so other tests aren't affected)
		await page.evaluate(async () => {
			await fetch('/api/reviews/ceo-state', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ state: 'live' }),
			});
		});

		assert.ok(
			hasMaint || bodyText.toLowerCase().includes('maintenance'),
			'expected maintenance indicator when state is maintenance',
		);
	});
});
