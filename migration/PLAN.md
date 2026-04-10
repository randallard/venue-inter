# VenueInter Migration Plan

## Goal

Replace the Dioxus/SSR frontend with a SvelteKit + TypeScript frontend (matching
the juryinter target architecture), while retaining the Axum backend for Informix
database access.

**This project is the proof-of-concept for the juryinter migration.** All
architectural decisions, phase structure, and tooling choices made here are
intended to be validated and then applied to juryinter.

## Source Application

VenueInter currently runs a Dioxus 0.7 fullstack frontend over the same Axum
backend. The backend already handles OIDC auth, Informix ODBC queries, Postgres
app-state, tasks, tickets, and email. Only the frontend changes.

## Target Architecture

```
venue-inter/
├── frontend/                    # SvelteKit + TypeScript (pnpm)
│   ├── src/
│   │   ├── lib/
│   │   │   ├── api.ts          # Typed fetch wrapper
│   │   │   ├── types.ts        # All TypeScript interfaces
│   │   │   ├── toast.svelte.ts # Toast notifications
│   │   │   └── components/     # Reusable UI components
│   │   └── routes/
│   │       ├── +layout.svelte  # Root layout (navbar, auth)
│   │       ├── dashboard/      # Status monitoring
│   │       ├── pools/          # Pool management
│   │       ├── reviews/        # CEO review workflow
│   │       ├── reports/        # Reports
│   │       └── data/           # Query browser
│   ├── tests/                  # Puppeteer E2E tests
│   ├── package.json
│   ├── pnpm-lock.yaml
│   ├── tsconfig.json
│   ├── vite.config.ts
│   └── svelte.config.js
├── crates/
│   ├── server/                 # Axum backend (extended with new API routes)
│   └── shared-types/           # Rust types (mirrored in types.ts)
├── ifx-config/                 # Informix ODBC config and seed data
├── migrations/                 # Postgres schema
└── migration/                  # This folder
```

## UI Organization

```
NavBar:  Dashboard │ Pools │ Reviews │ Reports │ Data        [user]

1. Dashboard (/)
   ├── Bad show code status
   ├── Blank questionnaire status
   └── Participant portal lockout status

2. Pools (/pools)
   ├── Active pools
   ├── Participant draw
   ├── Seating charts
   ├── Supplemental questionnaires
   └── Participant lookup

3. Reviews (/reviews)
   ├── Excuse review (admin queue)          ← admin preps and sends to CEO
   ├── Disqualification review (admin queue)
   ├── CEO review queue (ceo-review group)  ← combined scrollable page, all cases inline
   ├── Review records / history
   └── Sync status (/reviews/sync)          ← admin: cross-system Informix/PG/queue health

4. Reports (/reports)
   ├── Race / gender demographics
   ├── Address verification (NCOA)
   └── Participation history

5. Data (/data)
   └── Dynamic query browser
```

## Role System

| Group (Authentik) | Access |
|---|---|
| `users` | Standard access — all sections |
| `helpdesk` | Standard access + all tickets |
| `ceo-review` | **Narrow view only** — CEO review queue (`/reviews/ceo`). No access to Dashboard, Pools, Reports, or Data. Admin must prep and send a record before it appears here. |

## Tech Stack

| Concern | Choice | Rationale |
|---|---|---|
| Frontend framework | SvelteKit 2 + Svelte 5 | Matches juryinter target |
| Language | TypeScript (strict mode) | Type safety |
| Package manager | pnpm | Matches juryinter |
| Build tool | Vite | Comes with SvelteKit |
| API layer | Typed `apiFetch<T>` wrapper | Matches juryinter pattern |
| Styling | CSS custom properties | Shared design system |
| E2E testing | Puppeteer | Verifiable by developer and AI |
| Backend | Axum (existing) | Already connects to Informix via ODBC |
| Auth | OIDC via Authentik (existing) | Already implemented |
| Dev proxy | Vite `server.proxy` | `/api/*` and `/auth/*` to `:8080` |

## Informix Domain

The venueinter Informix database contains:

| Table | Description |
|---|---|
| `participant` | People on the audience list (corporate-loaded every 2 years) |
| `pool` | A draw group for a specific show |
| `pool_member` | Participant status within a pool (status 1/2/5/6/7) |
| `show` | Upcoming events needing audience |
| `show_type` | Show type lookup |
| `venue` | Venue locations |
| `part_history` | Audit trail of status changes |
| `review_record` | Pending excuse / disqualification records |

Pool member status codes (mirror UJMS):
- `1` = in pool
- `2` = qualified / selected
- `5` = permanently excused
- `6` = disqualified
- `7` = temporarily excused

## Postgres Tables

Beyond the base tasks/tickets/sessions:

| Table | Description |
|---|---|
| `status_reviews` | CEO review queue — admin sends prepped cases here |
| `review_history` | Full audit trail of every review action |
| `app_config` | Feature flags (`ceo_review_state`: live/maintenance) |
| `informix_sync_queue` | Async write-back queue: CEO decisions queue Informix updates here; processed by Phase 7 cron |
| `document_cache` | WebDAV document cache — TIF files fetched from national system and stored as BYTEA; keyed by `webdav_path` |

## Phase Completion Summary

| Phase | Status | Notes |
|---|---|---|
| 1 — Foundation | Complete | Auth, navbar, data browser, Puppeteer harness |
| 2 — Dashboard | Mostly complete | All remediation pages built; sync failure detail view not built; no E2E tests |
| 3 — Pool Management | Partial | Pools listing built; draw, seating, participant lookup not built |
| 4 — Questionnaires | Not started | Doc access covered by document cache (Phase 5 ext.) |
| 5 — CEO Review | Complete+ | All review workflow built; document caching and sync status added beyond plan |
| 6 — Reports | Not started | Landing page placeholder only |
| 7 — Background Processing | Mostly complete | Sync queue + review refresh crons built; participant sync and ticket-on-failure not built |
| 8 — Testing | Partial | smoke, data-browser, reviews tests written; dashboard/pools/reports/background tests pending |

---

## Phased Approach

### Phase 1: Foundation
SvelteKit scaffold, auth flow, navbar, data browser.
See [01-foundation.md](01-foundation.md)

### Phase 2: Dashboard
Status monitoring — bad show codes, blank questionnaires, portal lockouts.
See [02-dashboard.md](02-dashboard.md)

### Phase 3: Pool Management
Active pools, participant lookup, draw, seating charts.
See [03-pools.md](03-pools.md)

### Phase 4: Questionnaires
Eligibility form generation, distribution, and upload.
See [04-questionnaires.md](04-questionnaires.md)

### Phase 5: CEO Review
Admin prep queue + CEO narrow review interface.
See [05-reviews.md](05-reviews.md)

### Phase 6: Reports
Demographics, address verification, participation history.
See [06-reports.md](06-reports.md)

### Phase 7: Background Processing
Cron jobs, task queues, data synchronization.
See [07-background-processing.md](07-background-processing.md)

### Phase 8: Testing Strategy
End-to-end test coverage and CI integration.
See [08-testing-strategy.md](08-testing-strategy.md)

## Development Workflow

1. Pick the next chunk from the current phase detail doc
2. Implement the backend API endpoint (Rust/Axum)
3. Add the TypeScript types to `types.ts`
4. Build the SvelteKit page/component
5. Write a Puppeteer test that verifies the feature
6. Run `pnpm check` for type safety
7. Run Puppeteer test suite
8. Developer verifies locally
9. Commit and move to next chunk
