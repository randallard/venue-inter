# Phase 8: Testing Strategy

## Overview

Testing at three layers:

| Layer | Tool | When |
|---|---|---|
| Type safety | `pnpm check` (svelte-check + tsc) | Every commit |
| API contract | `curl` / manual | During each phase chunk |
| E2E behavior | Puppeteer | Each phase chunk, full suite before merge |

## Test File Organization

```
frontend/tests/
├── helpers.ts          # Browser setup, login, teardown
├── smoke.test.ts       # Phase 1: login, nav, data browser
├── dashboard.test.ts   # Phase 2: status cards, remediation
├── pools.test.ts       # Phase 3: pool list, draw, seating, lookup
├── questionnaires.test.ts  # Phase 4: questionnaire workflow
├── reviews.test.ts     # Phase 5: admin queue, CEO queue, decisions
├── reports.test.ts     # Phase 6: all three reports
└── background.test.ts  # Phase 7: task execution
```

## Puppeteer Conventions

```typescript
// helpers.ts
const BASE_URL = process.env.TEST_URL || 'http://localhost:5173';

export async function loginAs(page: Page, role: 'admin' | 'ceo') {
  // Authenticate as devuser (admin) or ceouser (CEO-only role)
  // Uses Authentik dev credentials from docker-compose
}

export async function expectText(page: Page, selector: string, text: string) {
  const el = await page.waitForSelector(selector);
  const content = await el?.evaluate(e => e.textContent);
  expect(content).toContain(text);
}
```

## Role Testing

Every restricted route must have a test verifying the role gate:

```typescript
// reviews.test.ts
test('CEO queue is inaccessible to non-CEO users', async () => {
  await loginAs(page, 'admin');
  await page.goto(`${BASE_URL}/reviews/ceo`);
  // Should redirect to / or show 403
  expect(page.url()).not.toContain('/reviews/ceo');
});

test('Admin cannot access CEO decision view', async () => {
  await loginAs(page, 'admin');
  await page.goto(`${BASE_URL}/reviews/ceo/some_part_key`);
  expect(page.url()).not.toContain('/reviews/ceo');
});
```

## Data Assertions

Tests assert against **specific seed data values** so regressions in queries
are caught immediately:

```typescript
test('Active pools shows Theater and Sports pools', async () => {
  await loginAs(page, 'admin');
  await page.goto(`${BASE_URL}/pools`);
  await page.waitForSelector('table tbody tr');
  const rows = await page.$$('table tbody tr');
  expect(rows.length).toBe(2);
  await expectText(page, 'table', 'THEATER');
  await expectText(page, 'table', 'SPORTS');
});
```

## CI Integration

Add to `frontend/package.json`:
```json
{
  "scripts": {
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test:e2e": "npx tsx tests/smoke.test.ts",
    "test:all": "npx tsx tests/smoke.test.ts && npx tsx tests/pools.test.ts && ..."
  }
}
```

CI pipeline runs `pnpm check && pnpm test:all` against a running dev stack.

## Lessons for juryinter

Document any deviations from the planned approach discovered during venue-inter
testing in a `migration/LESSONS.md` file. This becomes the input to the
juryinter migration planning.
