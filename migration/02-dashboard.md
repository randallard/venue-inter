# Phase 2: Dashboard

## Goal

Build the monitoring dashboard showing status indicators for operational issues:
bad show codes, blank questionnaires, participant portal lockouts, Informix sync
queue health, and stale pending-CEO records. Each indicator links to a
remediation page.

## Routes

| Route | Description |
|---|---|
| `/` | Dashboard with status cards |
| `/pools/fix-show-codes` | Review and fix bad show/division codes |
| `/pools/reset-questionnaire` | Reset blank qualification questionnaires |
| `/pools/lockouts` | Participant portal lockout management |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/dashboard/status` | GET | Counts for all status cards |
| `/api/pools/fix-show-codes` | GET | List pool members with bad show codes |
| `/api/pools/fix-show-codes` | POST | Batch-fix bad show codes |
| `/api/pools/reset-qq` | POST | Reset a blank questionnaire for one participant |
| `/api/pools/lockouts` | GET | List locked participant portal accounts |
| `/api/pools/unlock` | POST | Unlock a participant portal account |

## TypeScript Types

```typescript
interface DashboardStatus {
  bad_show_codes: number;
  blank_questionnaires: number;
  portal_lockouts: number;
  informix_sync_pending: number;   // green if 0, yellow with count if > 0
  informix_sync_failed: number;    // green if 0, red with count if > 0
  stale_pending_ceo: number;       // green if 0, yellow with count if > 0
}

interface BadShowCodeRow {
  pm_id: number;
  pool_no: number;
  part_no: number;
  fname: string | null;
  lname: string | null;
  current_code: string | null;
}

interface PortalLockoutRow {
  part_no: number;
  fname: string | null;
  lname: string | null;
  locked_at: string | null;
}
```

## Status Cards

| Card | Source | Green | Yellow | Red |
|---|---|---|---|---|
| Bad Show Codes | Informix | 0 | — | > 0 |
| Blank Questionnaires | Informix | 0 | — | > 0 |
| Portal Lockouts | Informix | 0 | — | > 0 |
| Informix Sync Pending | PG `informix_sync_queue` | 0 | > 0 | — |
| Sync Failures | PG `informix_sync_queue` | 0 | — | > 0 |
| Stale CEO Records | PG `status_reviews.data_stale` | 0 | > 0 | — |

**Stale CEO Records card:** Count of `status_reviews` rows where `data_stale = true`
and `status = 'pending_ceo'`. Yellow when > 0. Clicking the card links to
`/reviews/excuse` and `/reviews/disqualify` admin queues where the stale banner
surfaces the affected records. There is no separate remediation page — the admin
review screen is the remediation path (recall → reconcile → re-send).

## Chunks

### 2.1 Backend: Dashboard status API

`GET /api/dashboard/status` — three sources:

- Informix: counts of bad show codes, blank questionnaires, portal lockouts
- PostgreSQL `informix_sync_queue`: pending and failed counts
- PostgreSQL `status_reviews`: count of `data_stale = true AND status = 'pending_ceo'`

```sql
-- Sync queue counts
SELECT
  COUNT(*) FILTER (WHERE status = 'pending') AS informix_sync_pending,
  COUNT(*) FILTER (WHERE status = 'failed')  AS informix_sync_failed
FROM informix_sync_queue
WHERE status != 'completed';

-- Stale CEO records
SELECT COUNT(*) AS stale_pending_ceo
FROM status_reviews
WHERE data_stale = true AND status = 'pending_ceo';
```

**Verify:** Response matches expected counts from seed data. Stale count is 0
before the staleness cron has run.

### 2.2 Build dashboard page

`/` — six status cards in a grid. Colors per the table above. Stale CEO Records
card links to the admin review queues.

**Verify:** All six cards render with correct counts and colors.

### 2.3 Backend: Bad show codes API

`GET /api/pools/fix-show-codes` — list `pool_member` rows where `div_code`
doesn't match an entry in `show_type`.

**Verify:** Returns rows matching bad codes in seed data.

### 2.4 Build bad show codes page

`/pools/fix-show-codes` — table of bad records with "Fix All" button.
Confirmation modal before executing. Dashboard badge updates after fix.

**Verify:** Fix action updates records. Dashboard count changes to 0.

### 2.5 Backend + page: Reset questionnaire

`POST /api/pools/reset-qq` — reset blank questionnaire status for one participant.
`/pools/reset-questionnaire` — search by part_no, confirmation step before reset.

**Verify:** Reset action executes. Dashboard count updates.

### 2.6 Backend + page: Portal lockouts

`GET /api/pools/lockouts` — list locked portal accounts.
`POST /api/pools/unlock` — unlock one account.
`/pools/lockouts` — list with unlock button per row.

**Verify:** Unlock executes. Row disappears from list.

### 2.7 Write E2E tests

1. Dashboard loads with all six status cards showing correct counts
2. Stale CEO Records card shows 0 initially; correct count after cron flags a record
3. Bad codes list renders, fix action clears dashboard badge
4. QQ reset executes, confirmation shown
5. Lockout list renders, unlock action completes
6. Stale card links navigate to correct admin review queues

**Verify:** All tests pass.

## Implementation Status

### Backend
- [x] `GET /api/dashboard/status` — bad show codes, blank QQs, portal lockouts, sync pending, sync failed
- [x] `GET/POST /api/pools/fix-show-codes`
- [x] `GET /api/pools/blank-questionnaires`
- [x] `POST /api/pools/reset-qq`
- [x] `GET /api/pools/lockouts`
- [x] `POST /api/pools/unlock`
- [ ] `stale_pending_ceo` count in dashboard status — not added (depends on Phase 7 schema changes)

### Frontend
- [x] `/` — dashboard with five status cards (bad codes, blank QQs, lockouts, sync pending, sync failed)
- [x] `/pools/fix-show-codes`
- [x] `/pools/reset-questionnaire`
- [x] `/pools/lockouts`
- [ ] Stale CEO Records card — not built
- [ ] Sync failure detail view — not built; use `/reviews/sync` instead

### Testing
- [ ] `dashboard.test.ts` — not written

## Exit Criteria

- [x] Dashboard shows correct status counts for existing five cards
- [x] Each remediation action works end-to-end
- [ ] Stale CEO Records card added (depends on Phase 7 `data_stale` column)
- [ ] Dashboard badges update after remediation (verify locally)
- [ ] `dashboard.test.ts` Puppeteer tests written
- [ ] Developer has verified each workflow