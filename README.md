# VenueInter — Venue Audience Management System

A fullstack Rust application for managing venue audience participants, built with
[Dioxus](https://dioxuslabs.com/) 0.7 (transitioning to SvelteKit — see
[migration/](migration/)) and backed by IBM Informix.

VenueInter manages audience participant pools: corporate loads participant data
every two years, staff creates pools for upcoming shows, draws eligible
participants, tracks eligibility questionnaires, and routes excuse/disqualification
cases through an admin → CEO review workflow.

## Migration Story

This project is the **proof-of-concept for the juryinter migration**. The current
phase bridges the existing Informix database while the SvelteKit frontend is built
out phase by phase. See [migration/PLAN.md](migration/PLAN.md) for the full plan.

## Architecture

Cargo workspace with three crates:

```
crates/
  app/           # Dioxus fullstack entry point (SSR + WASM hydration) — being replaced
  server/        # Backend logic: auth (OIDC), database (Informix ODBC), API handlers
  shared-types/  # Models shared between app and server
```

- **Frontend**: Dioxus 0.7 (transitioning to SvelteKit + TypeScript)
- **Backend**: Axum 0.8 with custom routes merged into the Dioxus router
- **Auth**: OpenID Connect via Authentik; group-based RBAC (`ceo-review` group for narrow CEO view)
- **Informix Database**: Participant data via ODBC (odbc-api crate, requires IBM CSDK)
- **PostgreSQL Database**: Tasks, tickets, CEO review queue (`status_reviews`), sessions
- **Email**: Optional SMTP failure notifications via lettre

## Role System

| Group | Access |
|---|---|
| `users` | Standard access — all sections |
| `helpdesk` | Standard + all tickets |
| `ceo-review` | **Narrow view only** — CEO review queue (`/reviews/ceo`) after admin sends prepped cases |

## Docker Compose Services

| Service | Image | Port | Purpose |
|---|---|---|---|
| `informix-dev` | `icr.io/informix/informix-developer-database` | 9088 | Informix with participant seed data |
| `venueinter-db` | `postgres:16-alpine` | 5433 | App PostgreSQL — tasks, tickets, reviews, sessions |
| `authentik-server` | `ghcr.io/goauthentik/server:2025.10` | 9000, 9443 | OIDC provider |
| `authentik-worker` | `ghcr.io/goauthentik/server:2025.10` | — | Authentik background worker |
| `authentik-db` | `postgres:16-alpine` | — | Authentik backing store |
| `venueinter` | Built from `Dockerfile` | 8080 | The Dioxus application |

## Getting Started

See **[docs/dev-setup.md](docs/dev-setup.md)** for the full local development
guide.

**Quick start** (Docker only):

```bash
cp .env.example .env
docker compose up --build
```

Visit http://localhost:8080. Informix takes ~90 s on first start.

### Dev credentials

| Role | Username | Password |
|---|---|---|
| Admin (full access) | `devuser` | `dev-password` |
| CEO (review only) | `ceouser` | `dev-password` |
| Authentik admin | `akadmin` | `dev-admin-password` |

## Informix Schema (`ifx-config/seed.sql`)

- `participant` — audience list (corporate-loaded every 2 years)
- `pool` — draw group for a specific show
- `pool_member` — participant status in a pool (status 1/2/5/6/7)
- `show`, `show_type`, `venue` — show and venue reference data
- `part_history` — status change audit trail
- `review_record` — pending excuse / disqualification records

## PostgreSQL Schema (`migrations/init.sql`)

- `tasks` — background task tracking
- `tickets` — failure tickets
- `status_reviews` — CEO review queue (admin sends prepped cases; CEO decides)
- `review_history` — full audit trail of review actions
- `app_config` — feature flags (CEO review live/maintenance state)
- `tower_sessions` — persistent session store (auto-created at startup)

## Security

FISMA High controls:

- OIDC authorization code flow with PKCE + CSRF
- PostgreSQL-backed sessions; `SameSite=Strict`, `HttpOnly`, `__Host-` prefix; 8-hour timeout
- Auth routes rate-limited (2 req/s per IP, burst 5)
- Structured audit logging under `audit` tracing target
- Security headers on all responses
- `Cache-Control: no-store` on all protected API responses
- Group-based RBAC via OIDC claims
