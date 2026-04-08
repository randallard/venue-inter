import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import type { Browser, Page } from 'puppeteer';
import { setup, teardown, login, BASE_URL } from './helpers.ts';

describe('data browser', () => {
	let browser: Browser;
	let page: Page;

	before(async () => {
		({ browser, page } = await setup());
		await login(page);
	});

	after(async () => {
		await teardown(browser);
	});

	it('query list loads at /data', async () => {
		await page.goto(BASE_URL + '/data');
		await page.waitForSelector('.feature-list a', { timeout: 10_000 });
		const links = await page.$$('.feature-list a');
		assert.ok(links.length > 0, 'expected at least one query link');
	});

	it('master table renders headers and rows', async () => {
		await page.goto(BASE_URL + '/data');
		await page.waitForSelector('.feature-list a');
		await page.click('.feature-list a:first-child');

		await page.waitForSelector('table thead th', { timeout: 10_000 });

		const headers = await page.$$eval('table thead th', ths =>
			ths.map(th => th.textContent?.trim()).filter(Boolean)
		);
		assert.ok(headers.length > 0, 'expected at least one column header');

		const rows = await page.$$('table tbody tr:not(.skeleton-row)');
		assert.ok(rows.length > 0, 'expected at least one data row');
	});

	it('page heading shows total count and current page', async () => {
		const heading = await page.$eval('.count-label', el => el.textContent ?? '');
		assert.match(heading, /Page 1 of \d+/);
	});

	it('pagination: next moves to page 2, prev returns to page 1', async () => {
		const nextBtn = await page.$('.pagination .btn-secondary:last-child');
		const isDisabled = await nextBtn?.evaluate(el => (el as HTMLButtonElement).disabled);

		if (isDisabled) {
			// Single-page result set — skip pagination steps
			return;
		}

		await nextBtn!.click();
		await page.waitForFunction(
			() => document.querySelector('.page-info')?.textContent?.includes('Page 2'),
			{ timeout: 5_000 }
		);
		assert.match(
			await page.$eval('.page-info', el => el.textContent ?? ''),
			/Page 2 of \d+/
		);

		await page.click('.pagination .btn-secondary:first-child');
		await page.waitForFunction(
			() => document.querySelector('.page-info')?.textContent?.includes('Page 1'),
			{ timeout: 5_000 }
		);
		assert.match(
			await page.$eval('.page-info', el => el.textContent ?? ''),
			/Page 1 of \d+/
		);
	});

	it('clicking a detail link opens the detail view', async () => {
		const detailLink = await page.$('table tbody a');
		if (!detailLink) {
			await page.goto(BASE_URL + '/data');
			await page.waitForSelector('.feature-list a');
			await page.click('.feature-list a:first-child');
			await page.waitForSelector('table tbody a', { timeout: 10_000 });
		}

		await page.click('table tbody a');
		await page.waitForSelector('.back-link', { timeout: 10_000 });

		assert.match(page.url(), /\/data\/[^/]+\/[^/]+/, 'expected detail URL pattern');

		await page.waitForSelector('table', { timeout: 5_000 });
		const headers = await page.$$eval('table thead th', ths => ths.length);
		assert.ok(headers > 0, 'expected detail table to have columns');
	});

	it('back link returns to master list', async () => {
		await page.click('.back-link');
		await page.waitForSelector('table thead th', { timeout: 10_000 });

		assert.doesNotMatch(
			page.url(),
			/\/data\/[^/]+\/[^/]+/,
			'expected to be back on master list, not detail'
		);
	});
});
