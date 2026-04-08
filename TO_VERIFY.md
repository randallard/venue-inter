# Verification Checklist

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
