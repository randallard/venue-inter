# Phase 1: Foundation

## Goal

Set up the SvelteKit + TypeScript frontend, establish the development patterns,
migrate the existing data browser, and create the Puppeteer test harness.
Everything built here becomes the scaffold for all subsequent phases.

## Chunks

### 1.1 Scaffold SvelteKit project

```bash
cd venue-inter
pnpm create svelte@latest frontend
cd frontend
pnpm install
```

**Files created:**
- `frontend/package.json`
- `frontend/svelte.config.js` — adapter-auto, vitePreprocess
- `frontend/tsconfig.json` — strict mode, bundler resolution

**Verify:** `pnpm dev` starts on `:5173`, shows default SvelteKit page.

---

### 1.2 Configure TypeScript and Vite proxy

**`tsconfig.json`:**
```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

**`vite.config.ts`:**
```typescript
export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:8080',
      '/auth': 'http://localhost:8080'
    }
  }
});
```

**Verify:** `pnpm check` passes with zero errors.

---

### 1.3 Set up CSS design system

Copy the design system from juryinter's `app.css`.

**Key tokens:** Navy `#0a2240`, gold `#b5985a`, Source Sans 3 font.

**Components:** `.navbar`, `.card`, `.table-wrap`, `table`, `.btn`, `.badge`,
`.pagination`, `.status-badge`, `.toast`, `.modal-overlay`.

**Verify:** Visual inspection — navbar renders, tables are styled.

---

### 1.4 Implement typed fetch wrapper

`src/lib/api.ts`:

```typescript
class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) { ... }
}

async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, { credentials: 'include', ...init });
  if (res.status === 401) {
    window.location.href = '/auth/login?return_to=' + encodeURIComponent(window.location.pathname);
    throw new ApiError(401, 'Not authenticated');
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new ApiError(res.status, body.error);
  }
  return res.json();
}
```

**Verify:** `pnpm check` passes.

---

### 1.5 Define initial types

`src/lib/types.ts` mirroring Rust `shared-types`:

```typescript
export interface UserSession {
  sub: string;
  email: string | null;
  name: string | null;
  groups: string[];
  authenticated_at: number;
}

export interface ParticipantRow {
  part_no: number;
  fname: string | null;
  lname: string | null;
  city: string | null;
  state: string | null;
  gender: string | null;
  race_code: string | null;
  active: string | null;
  date_added: string | null;
}

export interface PoolRow {
  pool_no: number;
  show_no: number | null;
  ret_date: string | null;
  div_code: string | null;
  office: string | null;
  capacity: number | null;
  member_count: number;
}
```

**Verify:** `pnpm check` passes.

---

### 1.6 Build NavBar component

`src/lib/components/NavBar.svelte`:

```
[VenueInter] [badge]    Dashboard  Pools  Reviews  Reports  Data    [user] [Logout]
```

- CEO role (`ceo-review` group) sees **only** `/reviews/ceo` — no other nav items
- Instance badge (LIVE/TEST/TRAIN) from `APP_INSTANCE`
- Active link highlighting based on current route

**Verify:** Visual inspection. CEO user sees only the review link.

---

### 1.7 Implement auth flow

- `+layout.ts` loads user session via `getCurrentUser()`
- If no session, show login prompt
- `apiFetch` auto-redirects to login on 401
- CEO users redirected to `/reviews/ceo` on login

**Verify:** Login/logout round-trip works. CEO role routing enforced.

---

### 1.8 Build root layout

`src/routes/+layout.svelte` — renders NavBar + Toast globally.

`src/routes/+layout.ts`:
```typescript
export const ssr = false;
export async function load() {
  try {
    const user = await getCurrentUser();
    return { user };
  } catch {
    return { user: null };
  }
}
```

**Verify:** Layout consistent across pages.

---

### 1.9 Migrate data browser

Port existing Axum query browser to SvelteKit:

- `src/routes/data/+page.svelte` — query link listing
- `src/routes/data/[slug]/+page.svelte` — paginated master table
- `src/routes/data/[slug]/[id]/+page.svelte` — detail records

Uses `queries.yaml` driven API (add this to Axum if not present, following juryinter pattern).

**Verify:** Navigate to `/data`, select a query, see paginated table, click row for detail.

---

### 1.10 Set up Puppeteer test harness

```bash
cd frontend
pnpm add -D puppeteer @types/node
```

**`tests/helpers.ts`** — setup, teardown, login helpers.

**`tests/smoke.test.ts`** — basic: page loads, login works, navbar renders.

**Verify:** `pnpm test:e2e` passes.

---

### 1.11 Write foundation E2E tests

1. Login flow
2. NavBar — correct sections visible per role (CEO sees only review link)
3. Data browser — query list, table, pagination, detail
4. Logout
5. 401 handling

**Verify:** All tests green.

## Implementation Status

- [x] SvelteKit project scaffolded (`frontend/`)
- [x] Vite proxy: `/api` and `/auth` → `:8080`
- [x] `apiFetch<T>` typed wrapper with 401→login redirect
- [x] `types.ts` — all domain interfaces
- [x] CSS design system (navy/gold tokens, shared components)
- [x] NavBar — CEO role sees only `/reviews/ceo`, no other nav items
- [x] Auth flow — OIDC via Authentik, session cookie, `+layout.ts` loads user
- [x] CEO redirect: `+layout.ts` redirects CEO users from `/` to `/reviews/ceo`
- [x] Data browser — `/data`, `/data/[slug]`, `/data/[slug]/[id]`
- [x] Puppeteer harness — `tests/helpers.ts`, `tests/smoke.test.ts`, `tests/data-browser.test.ts`

## Exit Criteria

- [x] SvelteKit app runs on `:5173`, proxies API to `:8080`
- [x] Auth flow works end-to-end
- [x] CEO role sees only `/reviews/ceo` nav item
- [x] CEO redirect from `/` to `/reviews/ceo` enforced
- [x] Data browser functional
- [x] Puppeteer smoke + data-browser tests written
- [ ] `pnpm check` passes with zero errors (run to verify)
- [ ] Developer has manually verified all features
