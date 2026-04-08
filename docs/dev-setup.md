# VenueInter — Local Development Setup

This guide covers everything needed to run the full stack locally: the Informix
dev container, Authentik OIDC provider, Rust backend, and SvelteKit frontend.

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js | 20+ | [nodejs.org](https://nodejs.org) or `nvm` |
| pnpm | 9+ | `npm install -g pnpm` |
| Docker Engine | 24+ | distro package or [docs.docker.com](https://docs.docker.com/engine/install/) |
| Docker Compose | v2 | included with Docker Desktop; `docker compose version` to verify |

---

## 1. Obtain the Informix CSDK

The backend links against the IBM Informix Client SDK (CSDK) for ODBC. It is not
in git (~81 MB).

1. Extract from an IBM CSDK 4.50 installation. The tarball must contain:
   `lib/`, `bin/`, `incl/`, `msg/`, `gls/`
2. Place it at `csdk/csdk.tar.gz` (the `csdk/` directory is gitignored).

---

## 2. Configure the environment

```bash
cp .env.example .env
```

The defaults in `.env.example` work for the Docker Compose stack out of the box.

Key variables:

| Variable | Default | Notes |
|---|---|---|
| `INFORMIX_DSN` | `venueinter_dev` | Matches the DSN in `ifx-config/odbc-docker.ini` |
| `INFORMIX_USER` | `informix` | Informix container default |
| `INFORMIX_PASSWORD` | `in4mix` | Informix container default |
| `DATABASE_URL` | `postgres://venueinter:venueinter-dev-password@localhost:5433/venueinter` | Application PostgreSQL |
| `OIDC_ISSUER_URL` | `http://localhost:9000/application/o/venueinter/` | Local Authentik |
| `OIDC_CLIENT_ID` | `venueinter-dev-client-id` | Set by Authentik blueprint |
| `OIDC_CLIENT_SECRET` | `venueinter-dev-client-secret` | Set by Authentik blueprint |
| `COOKIE_SECURE` | `false` | Must be `false` for local HTTP; `true` behind HTTPS in prod |

---

## 3. Full stack via Docker Compose

```bash
docker compose up --build
```

First startup takes a few minutes:

- **Informix** (~90 s) — runs `setup.sh` post-init to create the schema and
  seed participant data. Watch for `IDS initialized` in the logs.
- **Authentik** (~30 s) — applies the OAuth2 blueprint (`venue-inter-oauth2.yaml`)
  on first boot, creating the `venueinter` application, dev users, and groups.
- **venueinter** — the Axum backend starts on port 8080.

Then start the frontend dev server:

```bash
cd frontend && pnpm install && pnpm dev
```

Visit **http://localhost:5173**. The Vite dev server proxies `/api` and `/auth`
to the backend at `:8080`.

```bash
docker compose down -v   # tear down and wipe all data volumes
```

### Dev credentials

| Role | Username | Password |
|---|---|---|
| Admin (full access) | `devuser` | `dev-password` |
| CEO (review queue only) | `ceouser` | `dev-password` |
| Authentik admin UI (`http://localhost:9000`) | `akadmin` | `dev-admin-password` |

---

## 4. Local dev — backend only (faster iteration)

Run infrastructure in Docker but the Axum backend locally for faster recompile.
Requires installing the CSDK on the host.

### 4a. Install CSDK locally

```bash
sudo mkdir -p /opt/informix
sudo tar xzf csdk/csdk.tar.gz -C /tmp
sudo cp -r /tmp/csdk/lib /tmp/csdk/bin /tmp/csdk/incl /tmp/csdk/msg /tmp/csdk/gls /opt/informix/
sudo rm -rf /tmp/csdk
sudo mkdir -p /opt/informix/etc
sudo cp ifx-config/sqlhosts-docker /opt/informix/etc/sqlhosts
sudo chown -R "$USER:$USER" /opt/informix
sudo chmod -R 755 /opt/informix
```

### 4b. Configure ODBC

```bash
sudo cp ifx-config/odbcinst.ini /etc/odbcinst.ini
sudo chmod 644 /etc/odbcinst.ini
cp ifx-config/odbc-docker.ini ~/.odbc.ini
```

### 4c. Add Informix env vars to your shell profile

```bash
export INFORMIXDIR=/opt/informix
export INFORMIXSERVER=informix
export INFORMIXSQLHOSTS=/opt/informix/etc/sqlhosts
export LD_LIBRARY_PATH=/opt/informix/lib:/opt/informix/lib/esql:/opt/informix/lib/cli${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export PATH=/opt/informix/bin:$PATH
export DB_LOCALE=en_US.819
export CLIENT_LOCALE=en_US.UTF8
```

Then reload: `source ~/.bashrc`

### 4d. Start only the infrastructure

```bash
docker compose up -d informix-dev venueinter-db authentik-server authentik-worker authentik-db
```

### 4e. Run the backend and frontend

```bash
# Terminal 1 — Axum backend
cargo run -p app

# Terminal 2 — SvelteKit frontend
cd frontend && pnpm dev
```

Backend listens on **http://localhost:8080**.
Frontend dev server at **http://localhost:5173**.

---

## 5. PostgreSQL schema

Sessions are auto-created by `PostgresStore::migrate()` at startup. All other
tables (`tasks`, `tickets`, `status_reviews`, `review_history`, `app_config`,
`informix_sync_queue`) are created from `migrations/init.sql`.

To add a new table:
1. Add DDL to `migrations/init.sql` with `IF NOT EXISTS` guards
2. Recreate the volume: `docker compose down -v && docker compose up -d venueinter-db`

---

## 6. Frontend development

The SvelteKit frontend lives in `frontend/`. It is a standalone pnpm project.

```bash
cd frontend
pnpm install          # install dependencies
pnpm dev              # start dev server at :5173
pnpm check            # TypeScript + svelte-check
pnpm build            # production build
```

The Vite config (`vite.config.ts`) proxies `/api/**` and `/auth/**` to the
backend at `http://localhost:8080`, so all API calls work transparently in dev.

---

## 7. Verifying services

```bash
# Informix health
docker compose exec informix-dev onstat -l

# Backend health endpoint
curl http://localhost:8080/health

# PostgreSQL tables
docker compose exec venueinter-db psql -U venueinter -c '\dt'

# Authentik — check blueprint was applied
# Visit http://localhost:9000 → Applications → should show "VenueInter"
```

---

## 8. Troubleshooting

**Backend fails with linker error or missing `libifcli.so`**

```bash
echo $LD_LIBRARY_PATH   # should include /opt/informix/lib
ls /opt/informix/lib/libifcli.so
```

**Informix container unhealthy**

```bash
docker compose logs --tail=50 informix-dev
# look for: IDS initialized
```

**Authentik login loop / OIDC discovery fails**

Visit `http://localhost:9000/application/o/venueinter/.well-known/openid-configuration`
— if 404, the blueprint hasn't applied. Check:

```bash
docker compose logs authentik-worker | grep blueprint
```

**Session cookie rejected**

Set `COOKIE_SECURE=false` in `.env` for local HTTP dev.

**Frontend shows blank page or 401**

Make sure the backend is running at `:8080`. The Vite proxy requires it to be up.

---

## SSL: External Informix

To connect to a production Informix requiring SSL:

1. Include `gskit/` and `ibm/` in the CSDK tarball
2. Create `config/conssl.cfg` pointing to your trust store
3. Place CA PEM files in `config/ssl/`
4. Change `Protocol=onsoctcp` → `onsocssl` in `crates/server/src/db.rs`
5. Change locales to `en_US.8859-1` to match the server

---

## Passkeys

Passkey support is available via `authentik-blueprints/venue-inter-passkeys.yaml`.
See [PASSKEY_SETUP_OPTIONS.md](../PASSKEY_SETUP_OPTIONS.md) for enrollment options.
