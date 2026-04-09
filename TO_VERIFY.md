# Verification Checklist

## How to run the automated E2E suite

```bash
cd frontend
TEST_USER=devuser TEST_PASSWORD=dev-password \
CEO_TEST_USER=ceouser CEO_TEST_PASSWORD=dev-password \
pnpm test:e2e
```

Requires the full Docker Compose stack to be up (`docker compose up`) and
the SvelteKit dev server running (`pnpm dev` in `frontend/`).

---

## Informix: Seed Data
- [ ] Rebuild Informix container: `docker compose down informix-dev && docker volume rm venue-inter_venue-ifx-data && docker compose up -d informix-dev`
- [ ] Verify participant table: `docker exec -it informix-dev bash -c 'echo "SELECT COUNT(*) FROM participant;" | dbaccess venueinter'`
- [ ] Verify pool data: `docker exec -it informix-dev bash -c 'echo "SELECT pool_no, ret_date, div_code FROM pool;" | dbaccess venueinter'`
- [ ] Verify pool members: `docker exec -it informix-dev bash -c 'echo "SELECT pool_no, COUNT(*) FROM pool_member GROUP BY pool_no;" | dbaccess venueinter'`
- [ ] Verify review records: `docker exec -it informix-dev bash -c 'echo "SELECT rr_id, part_no, pool_no, review_type, status FROM review_record;" | dbaccess venueinter'`

## Authentik: OIDC Groups
- [ ] Rebuild authentik containers: `docker compose down authentik-server authentik-worker && docker compose up -d authentik-server authentik-worker`
- [ ] Log in as `devuser` — verify groups `helpdesk` and `users` in session
- [ ] Log in as `ceouser` — verify group `ceo-review` in session
- [ ] Confirm CEO user is redirected / limited to review queue only

## Postgres: Schema
- [ ] Start Postgres: `docker compose up -d venueinter-db`
- [ ] Verify all tables: `docker exec -it venueinter-db psql -U venueinter -d venueinter -c '\dt'`
- [ ] Verify status_reviews schema: `docker exec -it venueinter-db psql -U venueinter -d venueinter -c '\d status_reviews'`
- [ ] Verify app_config has ceo_review_state: `docker exec -it venueinter-db psql -U venueinter -d venueinter -c 'SELECT * FROM app_config;'`

## App: Navigation
- [ ] All nav links work: Home, Participants, Pool Staff, Tasks, Tickets
- [ ] Home page shows all quick-links
- [ ] Auth guard redirects unauthenticated users to login
- [ ] No console errors in browser

## App: Participants Page
- [ ] Navigate to `/participants` — verify 20 participants load
- [ ] Active/Inactive status displays correctly (James Baker should show Inactive)

## App: Pool Staff + Tasks
- [ ] Navigate to `/pool-staff` — verify data loads
- [ ] Select a replacement, click "Replace All" → confirmation modal appears
- [ ] Confirm → "Task Started" modal with task ID
- [ ] Navigate to `/tasks` — task appears, status progresses pending → running → completed

## App: Failure Handling
- [ ] Force a failure (stop Informix mid-task)
- [ ] Verify task status shows "failed" with error detail
- [ ] Verify ticket created: `docker exec -it venueinter-db psql -U venueinter -d venueinter -c 'SELECT * FROM tickets;'`

## App: Tickets
- [ ] As `devuser` (helpdesk group) — navigate to `/tickets`, see all tickets
- [ ] As non-helpdesk user — see only own tickets

---

## Phase 5: CEO Review Workflow (manual verification)

### Accounts needed

| Account | Username | Password | Used for |
|---|---|---|---|
| Admin | `devuser` | `dev-password` | Admin queues, send-to-CEO, review history |
| CEO | `ceouser` | `dev-password` | CEO queue, CEO decision view |

### Admin queue — excuse requests

- [ ] Log in as `devuser`
- [ ] Navigate to `/reviews` — landing shows three queue cards with counts and a history lookup
- [ ] Navigate to `/reviews/excuse` — at least one record from seed data (part_no 7 pool 1, part_no 14 pool 2)
- [ ] Click a row — opens `/reviews/excuse/:part_key`
- [ ] Individual review shows: participant data panel, pool status panel, review record panel
- [ ] "View History" link appears in the nav row above the page content
- [ ] Admin notes textarea is editable
- [ ] "Send to CEO" button is present and enabled

### Admin queue — disqualification requests

- [ ] Navigate to `/reviews/disqualify` — at least one record (part_no 11 pool 1)
- [ ] Click a row — opens `/reviews/disqualify/:part_key`
- [ ] Same panels and Send to CEO button visible

### Send to CEO

- [ ] From an individual excuse review, enter a note and click **Send to CEO**
- [ ] Success message appears on the page
- [ ] Record status changes to `pending_ceo` in the page
- [ ] Navigating back to `/reviews/excuse` — that record no longer appears in the pending list

### CEO queue

- [ ] Log out and log in as `ceouser`
- [ ] Navigate to `/reviews/ceo` — the record just sent by admin appears
- [ ] Count badge shows correct number
- [ ] Clicking **Decide** on a row opens `/reviews/ceo/:part_key`

### CEO decision view

- [ ] Participant data and pool status panels render (read-only)
- [ ] Admin notes are visible
- [ ] CEO notes textarea is required (clicking any decision button without notes shows validation error)
- [ ] Enter notes and click **Re-qualify** (or another decision) — navigates back to `/reviews/ceo`
- [ ] The decided record no longer appears in the CEO queue
- [ ] Navigating to `/reviews/excuse` as `devuser` — the decided record shows as `completed`

### Review history

- [ ] Log in as `devuser`
- [ ] Navigate to `/reviews`
- [ ] Enter a participant number in the history lookup and click **View History**
- [ ] Redirects to `/reviews/records/:part_no`
- [ ] Timeline shows all actions (submitted, sent_to_ceo, decision) with timestamps and actor emails
- [ ] From an individual excuse/disqualify review page, click **View History** — same result
- [ ] For a participant with no history — shows "No review history found" empty state

### CEO maintenance mode

- [ ] As `devuser`, call `POST /api/reviews/ceo-state` with `{ "state": "maintenance" }`:
  ```bash
  curl -X POST http://localhost:8080/api/reviews/ceo-state \
    -H 'Content-Type: application/json' \
    -d '{"state":"maintenance"}' \
    --cookie-jar /tmp/cookies.txt --cookie /tmp/cookies.txt
  ```
  (or use the browser DevTools Network tab from a logged-in session)
- [ ] As `ceouser`, navigate to `/reviews/ceo` — shows maintenance mode card, not the queue
- [ ] Restore: `POST /api/reviews/ceo-state` with `{ "state": "live" }`
- [ ] CEO queue returns to normal

### E2E automated tests

Run after completing manual verification above (ensures the test suite agrees
with what you verified by hand):

```bash
cd frontend
TEST_USER=devuser TEST_PASSWORD=dev-password \
CEO_TEST_USER=ceouser CEO_TEST_PASSWORD=dev-password \
pnpm test:e2e
```

- [ ] All tests pass (0 failures)
