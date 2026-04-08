# Phase 2: Dashboard

## Goal

Build the monitoring dashboard showing status indicators for operational issues:
bad show codes, blank questionnaires, and participant portal lockouts. Each
indicator links to a remediation page.

## Routes

| Route | Description |
|---|---|
| `/` | Dashboard with three status cards |
| `/pools/fix-show-codes` | Review and fix bad show/division codes |
| `/pools/reset-questionnaire` | Reset blank qualification questionnaires |
| `/pools/lockouts` | Participant portal lockout management |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/dashboard/status` | GET | Count of bad codes, blank QQs, lockouts |
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
  informix_sync_pending: number;  // green if 0, yellow with count if > 0
  informix_sync_failed: number;   // green if 0, red with count if > 0
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

## Chunks

### 2.1 Backend: Dashboard status API

`GET /api/dashboard/status` — two sources:

- Informix: counts of bad show codes, blank questionnaires, portal lockouts
- PostgreSQL: `informix_sync_queue` counts by status

```sql
SELECT
  COUNT(*) FILTER (WHERE status = 'pending') AS informix_sync_pending,
  COUNT(*) FILTER (WHERE status = 'failed')  AS informix_sync_failed
FROM informix_sync_queue
WHERE status != 'completed';
```

**Verify:** Response matches expected counts from seed data.

### 2.1a Dashboard sync status cards

Two additional status cards below the existing three:

| Card | Condition | Color |
|---|---|---|
| Informix Sync — N pending | pending = 0 | green |
| Informix Sync — N pending | pending > 0 | yellow with count |
| Sync Failures — N failed | failed = 0 | green |
| Sync Failures — N failed | failed > 0 | red with count |

Pending count gives visibility into queue depth (cron may not have run yet).
Failed count is the actionable alert — clicking it opens a failure detail view
showing the `last_error` and payload for each failed row.

**Verify:** Cards update correctly as queue is populated and cleared.

### 2.2 Build dashboard page

`/` — three status cards with count badges. Each badge is red when non-zero,
green when clear. Cards link to respective remediation pages.

**Verify:** Counts display correctly. Colors change with data.

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

1. Dashboard loads with correct counts
2. Bad codes list renders, fix action clears dashboard badge
3. QQ reset executes, confirmation shown
4. Lockout list renders, unlock action completes

**Verify:** All tests pass.

## Exit Criteria

- [ ] Dashboard shows correct status counts
- [ ] Each remediation action works end-to-end
- [ ] Dashboard badges update after remediation
- [ ] All Puppeteer tests pass
- [ ] Developer has verified each workflow
