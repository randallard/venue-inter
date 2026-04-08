import puppeteer, { type Browser, type Page } from 'puppeteer';

export const BASE_URL = process.env.TEST_URL ?? 'http://localhost:5173';
const TEST_USER = process.env.TEST_USER ?? '';
const TEST_PASSWORD = process.env.TEST_PASSWORD ?? '';

export async function setup(): Promise<{ browser: Browser; page: Page }> {
	const browser = await puppeteer.launch({ headless: true });
	const page = await browser.newPage();
	return { browser, page };
}

export async function teardown(browser: Browser): Promise<void> {
	await browser.close();
}

/**
 * Log in via the Authentik OIDC flow.
 * Requires TEST_USER and TEST_PASSWORD environment variables.
 */
export async function login(page: Page): Promise<void> {
	if (!TEST_USER || !TEST_PASSWORD) {
		throw new Error('TEST_USER and TEST_PASSWORD env vars are required for E2E tests');
	}

	await page.goto(`${BASE_URL}/auth/login?return_to=/`);

	// Authentik username step
	await page.waitForSelector('input[name="uidField"], input[id="id_uid"]', { timeout: 15_000 });
	const uidSelector = (await page.$('input[name="uidField"]')) ? 'input[name="uidField"]' : 'input[id="id_uid"]';
	await page.type(uidSelector, TEST_USER);
	await page.keyboard.press('Enter');

	// Authentik password step
	await page.waitForSelector('input[type="password"]', { timeout: 10_000 });
	await page.type('input[type="password"]', TEST_PASSWORD);
	await page.keyboard.press('Enter');

	// Wait for redirect back to the app
	await page.waitForNavigation({ waitUntil: 'networkidle0', timeout: 20_000 });

	if (!page.url().startsWith(BASE_URL)) {
		throw new Error(`Login failed — ended up at: ${page.url()}`);
	}
}

/**
 * Clear cookies to simulate a logged-out session.
 */
export async function clearSession(page: Page): Promise<void> {
	const client = await page.createCDPSession();
	await client.send('Network.clearBrowserCookies');
}
