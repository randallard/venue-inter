# Phase 7: Background Processing

## Goal

Implement cron-driven background tasks for data synchronization, review queue
refresh, and stale record detection.

## Task Types

| Task Type | Trigger | Description |
|---|---|---|
| `sync_informix_queue` | Cron every 2 min | Process pending informix_sync_queue rows, write to Informix via ODBC |
| `refresh_review_queue` | Cron every 5 min | Pull latest Informix review_record data into Postgres status_reviews |
| `check_pending_ceo_staleness` | Cron every 15 min | Verify pending_ceo records against Informix; flag drifted records |

## Removed Tasks

`sync_participant_data` (nightly participant snapshot diff) has been removed from
the plan. Participant data is treated as live Informix reads throughout the app.
The verify/reconcile flow on the admin review screen (Phase 5) replaces this —
admins get per-record data freshness checks on demand rather than a batch nightly
diff log. The `informix_sync_queue` write-back remains the only direction where
local state flows back to Informix.

## Cron Schedule

```
*/2  * * * *   sync_informix_queue
*/5  * * * *   refresh_review_queue
*/15 * * * *   check_pending_ceo_staleness
```

Schedules executed by a Tokio task spawned at startup.

## Backend Changes

### 7.0 Informix sync queue task

Processes all `informix_sync_queue` rows with `status = 'pending'`, executing
the corresponding Informix ODBC write for each.

**Operations handled:**

| Operation | Informix write |
|---|---|
| `update_pool_member_status` | `UPDATE pool_member SET status = :new_status WHERE part_no = :part_no AND pool_no = :pool_no` |
| `close_review_record` | `UPDATE review_record SET status = 'C' WHERE part_no = :part_no AND pool_no = :pool_no` |
| `send_review_record` | `UPDATE review_record SET status = 'S' WHERE part_no = :part_no AND pool_no = :pool_no` |
| `reopen_review_record` | `UPDATE review_record SET status = 'P' WHERE part_no = :part_no AND pool_no = :pool_no` |

**Per-row logic:**
1. Execute the Informix write
2. On success: `UPDATE informix_sync_queue SET status = 'completed', completed_at = now()`
3. On failure: increment `attempts`, store `last_error`
4. After 3 failed attempts: set `status = 'failed'` — dashboard turns red for this row
5. On any failure: create a ticket with the error and payload for IT visibility

Failed rows are not retried automatically. Admin resolves the underlying issue
and manually re-queues or applies the fix.

**Document cache cleanup:** When all sync ops for a participant complete
successfully, delete corresponding `document_cache` rows:

```sql
DELETE FROM document_cache WHERE part_no = :part_no;
```

**Verify:** After a CEO decision creates two sync queue rows, running the task
updates Informix and marks both rows completed. Dashboard returns to green.
Running again is a no-op.

### 7.1 Review queue refresh task

Queries Informix `review_record` for records with `status IN ('P', 'S')` not
yet in `status_reviews`, and inserts them. Updates existing records if admin
notes have changed.

Does not overwrite records with `status = 'pending_ceo'` or `'completed'`.

### 7.2 Pending CEO staleness check task

For all `status_reviews` records with `status = 'pending_ceo'`, fire the same
Informix verify logic used by the admin review screen:

- Query Informix for all displayed participant fields
- Query `part_image` for current document identifiers
- Compare against Postgres data and `document_cache`

If any field differs or new documents exist:
- Set `data_stale = true`, `stale_detected_at = now()` on `status_reviews`
- Dashboard stale card count increments (yellow/red per count)
- Admin review screen banner appears for flagged records

If data matches and record was previously flagged:
- Clear `data_stale` and `stale_detected_at` (e.g. admin reconciled and re-sent)

**Verify:** After modifying a seed participant record in Informix, running the
cron flags the corresponding `pending_ceo` record. Dashboard card updates.
After reconcile and re-send, cron clears the flag on the next run.

### 7.3 Task result storage

All cron tasks write structured JSON to `tasks.result_summary`. Failures
create a `ticket` (existing pattern).

## Schema Changes

The following columns are added to `status_reviews` (`migrations/init.sql`):

```sql
ALTER TABLE status_reviews
  ADD COLUMN data_stale        BOOLEAN   NOT NULL DEFAULT FALSE,
  ADD COLUMN stale_detected_at TIMESTAMP,
  ADD COLUMN data_version      INTEGER   NOT NULL DEFAULT 0;
```

- `data_stale` — set by the staleness cron; cleared on reconcile or when cron
  confirms data matches again
- `stale_detected_at` — timestamp of first detection in the current stale window;
  useful for admin to judge urgency
- `data_version` — incremented by each reconcile; used by the CEO page poll to
  detect whether a record has been updated since the page loaded and show the
  per-record update banner

## Chunks

### 7.1 Informix sync queue cron

Implement `task_runner::spawn_sync_queue_cron`. Schedule in startup.

**Verify:** Sync queue rows processed, Informix updated, rows marked completed.
Document cache cleaned up on full completion. Dashboard reflects queue state.

### 7.2 Review queue refresh cron

Implement `task_runner::spawn_review_refresh_cron`. Schedule in startup.

**Verify:** New Informix review_record rows appear in status_reviews after cron
runs. Existing pending_ceo records not overwritten.

### 7.3 Pending CEO staleness cron

Implement `task_runner::spawn_staleness_check_cron`. Schedule in startup.

For each `pending_ceo` record:
1. Call the same verify logic as `GET /api/reviews/:part_key/verify`
2. Update `data_stale` / `stale_detected_at` accordingly
3. Log result to `tasks.result_summary`

**Verify:** Modified seed record is flagged within 15 minutes. Dashboard card
count is correct. Reconcile + re-send clears flag on next cron run.

### 7.4 Expose task status in UI

`/tasks` page shows cron task history with type labels and scheduled-task
indicators.

**Verify:** All three cron types appear in the task list with correct status
and result summaries.

### 7.5 Write E2E tests

1. Sync queue cron processes pending rows — Informix updated, rows completed
2. Review queue refresh — status_reviews populated from Informix
3. Staleness cron — flags pending_ceo record when Informix data differs;
   dashboard card count updates; flag cleared after reconcile
4. Task list shows all three cron types with correct history

**Verify:** All tests pass.

## Implementation Status

### Infrastructure
- [x] `informix_sync_queue` table exists; populated by `ceo_decide_handler`
- [x] `dashboard_status_handler` returns sync pending/failed counts
- [x] Sync status admin page (`/reviews/sync`)
- [ ] `data_stale`, `stale_detected_at`, `data_version` columns on `status_reviews` — not added

### Cron tasks (`crates/server/src/sync.rs`)
- [x] 7.0 `process_sync_queue` — drains queue, executes Informix writes, marks completed/failed; `reopen_review_record` operation **needs to be added**
- [x] 7.1 `refresh_review_queue` — idempotent insert of new Informix review_record rows
- [ ] 7.2 `check_pending_ceo_staleness` — not yet built
- [ ] 7.4 Cron task history in `/tasks` UI — not yet built

### Per-record sync API
- [x] `POST /api/reviews/sync-status/sync/:part_key`
- [x] Sync button in `/reviews/sync` UI

### Record lookup API
- [x] `GET /api/reviews/sync-status/lookup/:query`

## Exit Criteria

- [x] Informix sync queue task processes pending rows and writes to Informix
- [x] Review queue refresh runs and populates status_reviews from Informix
- [x] All implemented tasks are idempotent
- [x] Per-record sync trigger works from admin UI
- [ ] `reopen_review_record` operation added to sync queue task
- [ ] `data_stale`, `stale_detected_at`, `data_version` columns added to status_reviews
- [ ] Staleness cron built and scheduled
- [ ] Dashboard stale card reflects cron output
- [ ] Failed rows after 3 attempts create a ticket
- [ ] Task list UI shows cron history
- [ ] Developer has verified cron execution end-to-end