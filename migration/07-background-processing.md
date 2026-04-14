# Phase 7: Background Processing

## Goal

Implement cron-driven background tasks for data synchronization, report
pre-generation, and review queue refresh.

## Task Types

| Task Type | Trigger | Description |
|---|---|---|
| `sync_informix_queue` | Cron every 2 min | Process pending informix_sync_queue rows, write to Informix via ODBC |
| `refresh_review_queue` | Cron every 5 min | Pull latest Informix review_record data into Postgres status_reviews |
| `sync_participant_data` | Cron nightly | Detect participant record changes in Informix, log deltas |
| `generate_draw_export` | On-demand (API) | Generate Excel draw export file |
| `replace_staff` | On-demand (API) | Replace a staff member across future pool sessions |

## Cron Schedule

```
*/2 * * * *   sync_informix_queue
*/5 * * * *   refresh_review_queue
0   2 * * *   sync_participant_data
```

Schedules stored in environment config or a cron table; executed by a Tokio
task spawned at startup (following ifxinter's `task_runner` pattern).

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

**Per-row logic:**
1. Execute the Informix write
2. On success: `UPDATE informix_sync_queue SET status = 'completed', completed_at = now()`
3. On failure: increment `attempts`, store `last_error`
4. After 3 failed attempts: set `status = 'failed'` — dashboard turns red for this row
5. On any failure: create a ticket with the error and payload for IT visibility

Failed rows are **not retried automatically** after 3 attempts. An admin must
resolve the underlying issue and manually re-queue or apply the fix. The ticket
created on failure links to the specific sync row.

Dashboard reflects the live queue state: pending count (yellow), failed count (red).

**Idempotency:** Each row has a unique UUID. The task processes each row once.
Re-running the cron cannot double-apply a completed row.

**Document cache cleanup:** When all sync ops for a participant complete successfully
(i.e. all `informix_sync_queue` rows for that `part_no`/`pool_no` reach `status = 'completed'`),
delete the corresponding `document_cache` rows:

```sql
DELETE FROM document_cache WHERE part_no = :part_no;
```

This frees binary storage after the case is closed. Subsequent document views (e.g. from the
participant check page) fall back to the live WebDAV proxy without re-caching.

**Verify:** After a CEO decision creates two sync queue rows, running the task
updates Informix and marks both rows completed. Dashboard returns to green.
Running again is a no-op.

### 7.1 Review queue refresh task

Queries Informix `review_record` for any records with `status IN ('P', 'S')`
not yet in `status_reviews`, and inserts them. Updates existing records if
admin notes have changed.

Does **not** overwrite records with `status = 'pending_ceo'` or `'completed'`
(CEO queue takes precedence once a record has been sent).

### 7.2 Participant data sync task

Compares a snapshot of `participant` against a stored hash. Logs additions,
deactivations, and address changes to the task result. Does not modify data —
admin reviews the delta log.

### 7.3 Task result storage

Both task types write structured JSON to `tasks.result_summary` and create
a `ticket` on failure (existing pattern).

## Chunks

### 7.1 Review queue refresh cron

Implement `task_runner::spawn_review_queue_refresh`. Schedule in startup.

**Verify:** After seed data load, running the task populates `status_reviews`
with the Informix `review_record` rows. Running again is idempotent.

### 7.2 Participant sync cron

Implement `task_runner::spawn_participant_sync`. Schedule in startup.

**Verify:** Delta log produces correct output against seed data.

### 7.3 Expose task status in UI

`/tasks` page (already built in Phase 1) shows cron task history.
Add task type labels and scheduled-task indicators.

**Verify:** Cron tasks appear in the task list with correct status.

### 7.4 Write E2E tests

1. Review queue refresh runs — status_reviews populated
2. Participant sync runs — result_summary contains valid JSON
3. Task list shows cron tasks

**Verify:** All tests pass.

## Implementation Status

### Infrastructure
- [x] `informix_sync_queue` table exists (`migrations/init.sql`); populated by `ceo_decide_handler` (Phase 5)
- [x] `dashboard_status_handler` returns `informix_sync_pending` and `informix_sync_failed` counts
- [x] Sync status admin page (`/reviews/sync`) — per-record health badges, error details, sync queue state

### Cron tasks (`crates/server/src/sync.rs`)
- [x] 7.0 `process_sync_queue` — drains `informix_sync_queue`; executes Informix ODBC writes; marks rows completed/failed; after 3 attempts marks `status = 'failed'`; spawned every 2 min via `spawn_sync_queue_cron`
- [x] 7.1 `refresh_review_queue` — inserts new Informix `review_record` (status='P') rows into PG `status_reviews` (pending_admin); idempotent ON CONFLICT DO NOTHING; spawned every 5 min via `spawn_review_refresh_cron`
- [ ] 7.2 `sync_participant_data` nightly cron — not yet built
- [ ] 7.3 Cron task history in `/tasks` UI — not yet built

### Per-record sync API
- [x] `POST /api/reviews/sync-status/sync/:part_key` — immediately processes pending sync queue rows for one record; returns `{processed, succeeded, failed, errors[]}`
- [x] Sync button in `/reviews/sync` UI — triggers per-record sync, updates health badge inline

### Record lookup API
- [x] `GET /api/reviews/sync-status/lookup/:query` — accepts part_no or part_key; queries Informix review_record (all statuses, not just P) and PG; returns `SyncStatusResponse` — works for closed/processed records not in the main listing
- [x] Lookup input in `/reviews/sync` UI — shows results in a highlighted panel above the main table

## Exit Criteria

- [x] Informix sync queue task processes pending rows and writes to Informix
- [ ] Failed rows after 3 attempts set status = 'failed' and create a ticket (ticket creation not yet built)
- [x] Dashboard pending count reflects queue state
- [x] Review queue refresh runs and populates status_reviews from Informix
- [x] All implemented tasks are idempotent
- [x] Per-record sync trigger works from the admin UI
- [x] Lookup works for any participant regardless of queue state
- [ ] Participant sync runs and produces valid delta log
- [ ] Task failures create tickets
- [ ] Task list UI shows cron history
- [ ] Developer has verified cron execution end-to-end
