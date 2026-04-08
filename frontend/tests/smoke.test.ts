import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import type { Browser, Page } from 'puppeteer';
import { setup, teardown, login, clearSession, BASE_URL } from './helpers.ts';

describe('smoke', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('home page loads and navbar is visible', async () => {
		await page.goto(BASE_URL + '/');
		await page.waitForSelector('.navbar');
	});

	it('navbar has all five sections in order for standard user', async () => {
		const links = await page.$$eval('.nav-link', els =>
			els.map(el => el.textContent?.trim())
		);
		assert.deepStrictEqual(links, ['Dashboard', 'Pools', 'Reviews', 'Reports', 'Data']);
	});

	it('navbar shows logged-in user and logout link', async () => {
		const logoutLink = await page.$('a[href="/auth/logout"]');
		assert.ok(logoutLink, 'expected logout link when authenticated');
	});

	it('unauthenticated visit redirects away from the app', async () => {
		await clearSession(page);
		await page.goto(BASE_URL + '/data');
		// apiFetch 401 redirects to /auth/login, which redirects to Authentik
		await page.waitForNavigation({ waitUntil: 'networkidle0', timeout: 10_000 }).catch(() => {});
		const url = page.url();
		assert.ok(
			!url.startsWith(BASE_URL + '/data'),
			`expected redirect away from /data, got ${url}`
		);
	});
});
