# VenueInter — Venue Audience Management System

A fullstack application for managing venue audience participants. Backend is
Rust/Axum backed by IBM Informix (participant data) and PostgreSQL (app workflow).
Frontend is SvelteKit + TypeScript. See [migration/](migration/) for the
phase-by-phase build plan.

VenueInter manages audience participant pools: corporate loads participant data
every two years, staff creates pools for upcoming shows, draws eligible
participants, tracks eligibility questionnaires, and routes excuse/disqualification
cases through an admin → CEO review workflow.

## Architecture

```
crates/
  app/           # Axum server entry point (Dioxus fullstack shell; serves the SvelteKit build)
  server/        # Backend: auth (OIDC), Informix ODBC, PostgreSQL, API handlers, reviews
  shared-types/  # Serde models shared between crates

frontend/        # SvelteKit 2 + Svelte 5 + TypeScript — the actual UI
ifx-config/      # Informix schema + seed data for the dev container
migrations/      # PostgreSQL init schema (init.sql)
migration/       # Phase-by-phase build plan (Markdown specs)
```

- **Frontend**: SvelteKit 2 / Svelte 5 / TypeScript (strict), served at `:5173` in dev (Vite proxy to `:8080`)
- **Backend**: Axum 0.8, served at `:8080`
- **Auth**: OpenID Connect via Authentik; group-based RBAC
- **Informix**: All domain data (participant, pool, pool_member, review_record) via ODBC (odbc-api crate, requires IBM CSDK)
- **PostgreSQL**: App-local state — tasks, tickets, CEO review queue, sessions, sync queue

## Role System

| Group | Access |
|---|---|
| `users` | Full admin access — dashboard, pools, reviews, reports, data browser |
| `helpdesk` | Standard access + all tickets |
| `ceo-review` | **Narrow view only** — CEO review queue (`/reviews/ceo`); no other nav |

## What's Built

| Phase | Status | Description |
|---|---|---|
| 1 — Foundation | Done | Auth (OIDC + session), data browser (YAML-configured queries), participant/pool/staff views |
| 2 — Dashboard | Done | 5-card status dashboard: bad show codes, blank questionnaires, portal lockouts, sync pending/failed; inline fix flows |
| 5 — CEO Review | Done | Admin queues → send to CEO → CEO decision (async PG write, Informix via sync queue); full audit trail |
| 3 — Pool Mgmt | Planned | Pool creation, draw, status management |
| 4 — Questionnaires | Planned | Questionnaire tracking and reporting |
| 6 — Reports | Planned | Reporting and exports |
| 7 — Background | Planned | `informix_sync_queue` cron, replace_staff task |

## CEO Review Flow

1. Admin opens an excuse/disqualification request from the admin queue
2. Admin adds notes and clicks **Send to CEO** — record moves to `status_reviews` (PG, status=`pending_ceo`), Informix `review_record` status set to `S`
3. CEO sees only the prepared queue at `/reviews/ceo`
4. CEO enters notes and clicks a decision button (**Re-qualify / Disqualify / Perm Excuse / Temp Excuse / Send Back**)
5. Decision writes to PostgreSQL in a single transaction (fast, ~1–5 ms) — decision is durable on commit
6. Two `informix_sync_queue` rows are inserted (async); a cron job (Phase 7) applies them to Informix via ODBC

**Durability guarantee**: if the browser times out after a click, the page re-fetches before surfacing an error. If the PG transaction already committed, the browser treats it as success.

## Informix Schema (`ifx-config/seed.sql`)

| Table | Description |
|---|---|
| `participant` | Audience list — corporate-loaded every 2 years |
| `pool` | Draw group for a specific show |
| `pool_member` | Participant status in a pool (1=summoned, 2=qualified, 5=perm excuse, 6=disqualified, 7=temp excuse) |
| `show`, `show_type`, `venue` | Show and venue reference data |
| `part_history` | Informix-side status change audit trail |
| `review_record` | Pending excuse / disqualification records (status P/S/C) |
| `session_resources` | Pool session staff assignments |
| `staff_codes` | Staff code lookup table |

## PostgreSQL Schema (`migrations/init.sql`)

| Table | Description |
|---|---|
| `tasks` | Background task tracking (e.g. replace_staff) |
| `tickets` | Failure tickets — auto-created when sync queue items exhaust retries |
| `status_reviews` | CEO review queue; `part_key` (unique) links to Informix `part_no_pool_no` |
| `review_history` | Full audit trail of every review action (sent_to_ceo, decided, sent_back) |
| `app_config` | Runtime feature flags — see table below |
| `informix_sync_queue` | Deferred writes back to Informix; operations: `update_pool_member_status`, `close_review_record`, `reopen_review_record` |

### `app_config` keys

| Key | Values | Default | Effect |
|---|---|---|---|
| `ceo_review_state` | `live` \| `maintenance` | `live` | When `maintenance`: CEO queue returns empty and shows a banner — use to pause decisions during national system downtime |
| `show_review_notes` | `true` \| `false` | `false` | When `false`: hides all notes fields (admin prep textarea, CEO decision textarea, displayed admin/CEO notes) from the review queue UI |
| `show_send_back` | `true` \| `false` | `false` | When `false`: hides the admin **Recall to Admin** button and the CEO **Send Back** decision button |

Changes take effect immediately — the queue page reads config on every load. Update via SQL:

```sql
UPDATE app_config SET value = 'maintenance' WHERE key = 'ceo_review_state';
UPDATE app_config SET value = 'true'        WHERE key = 'show_review_notes';
UPDATE app_config SET value = 'true'        WHERE key = 'show_send_back';
```
| `tower_sessions` | Persistent session store (auto-created at startup by sqlx) |

## Docker Compose Services

| Service | Port | Purpose |
|---|---|---|
| `informix-dev` | 9088 | Informix with participant seed data |
| `venueinter-db` | 5433 | App PostgreSQL |
| `authentik-server` | 9000, 9443 | OIDC provider |
| `authentik-worker` | — | Authentik background worker |
| `authentik-db` | — | Authentik backing store |
| `venueinter` | 8080 | Rust backend (Axum) |

## Getting Started

See **[docs/dev-setup.md](docs/dev-setup.md)** for the full guide including CSDK setup and Authentik configuration.

**Quick start:**

```bash
cp .env.example .env
docker compose up --build        # starts Informix, PostgreSQL, Authentik, and the backend
cd frontend && pnpm install && pnpm dev   # SvelteKit dev server at :5173
```

Informix takes ~90 s on first start. Visit http://localhost:5173.

### Dev credentials

Both users are created automatically by the Authentik blueprint on first startup.

| Account | Username | Password | Authentik groups | Access |
|---|---|---|---|---|
| Admin | `devuser` | `dev-password` | `users`, `helpdesk` | Full app access |
| CEO | `ceouser` | `dev-password` | `users`, `ceo-review` | Full access + CEO review queue |
| Authentik admin | `akadmin` | `dev-admin-password` | — | Authentik UI at `:9000` only |

### Running E2E tests (Puppeteer)

```bash
cd frontend

# Admin tests only
TEST_USER=devuser TEST_PASSWORD=dev-password pnpm test:e2e

# Full suite including CEO queue and decision tests
TEST_USER=devuser TEST_PASSWORD=dev-password \
CEO_TEST_USER=ceouser CEO_TEST_PASSWORD=dev-password \
pnpm test:e2e
```

`CEO_TEST_USER`/`CEO_TEST_PASSWORD` are optional — CEO tests skip gracefully
when absent. See **[docs/dev-setup.md § 8](docs/dev-setup.md)** for full details.

### Running without Docker (backend only)

```bash
cargo run -p app          # backend at :8080
cd frontend && pnpm dev   # frontend at :5173
```

Requires Informix CSDK installed and `INFORMIX_*` env vars set. Set `COOKIE_SECURE=false` for local HTTP.

## Security

FISMA controls applied:

- OIDC authorization code flow with Authentik
- PostgreSQL-backed sessions — `SameSite=Strict`, `HttpOnly`, `__Host-` cookie prefix, 8-hour inactivity timeout
- Auth routes rate-limited: 2 req/s per IP, burst 5 (tower-governor)
- `Cache-Control: no-store` on all protected API responses
- Security headers on every response: `X-Content-Type-Options`, `X-Frame-Options: DENY`, `Strict-Transport-Security`, `Referrer-Policy`, `Permissions-Policy`
- Structured audit logging via `tracing` under the `audit` target
- Group-based RBAC enforced in middleware and per-handler
- CEO decision is a single PostgreSQL transaction — decision is WAL-durable on commit, never lost to a network failure
