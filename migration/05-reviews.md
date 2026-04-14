# Phase 5: CEO Review

## Goal

Build the two-stage review workflow: admin prep queue and CEO decision interface.

**Workflow (mirrors waed-jury-review admin→judge pattern):**
1. Admin receives an excuse or disqualification request
2. Admin pulls participant data, verifies questionnaire and documents are present
3. Admin clicks "Send to CEO" — record appears in CEO queue with status `pending_ceo`
4. CEO sees **only** prepped records — questionnaire, documents, and participant data
5. CEO makes a determination (or sends back to admin for more prep)

## Routes

| Route | Auth | Description |
|---|---|---|
| `/reviews` | users | Review landing — links to admin queues |
| `/reviews/excuse` | users | Admin excuse queue |
| `/reviews/excuse/[part_key]` | users | Individual excuse review (admin prep view) |
| `/reviews/disqualify` | users | Admin disqualification queue |
| `/reviews/disqualify/[part_key]` | users | Individual disqualification review |
| `/reviews/ceo` | ceo-review | CEO queue — only records admin has sent |
| `/reviews/ceo/[part_key]` | ceo-review | CEO decision view |
| `/reviews/records/[part_no]` | users | Review history for a participant |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/reviews/excuse/admin` | GET | Admin excuse queue |
| `/api/reviews/disqualify/admin` | GET | Admin disqualification queue |
| `/api/reviews/ceo` | GET | CEO queue (pending_ceo records only) |
| `/api/reviews/:part_key` | GET | Participant data + docs for review |
| `/api/reviews/:part_key/document` | GET | Scanned questionnaire document |
| `/api/reviews/:part_key/verify` | GET | Async Informix verify — returns field diff + document list delta |
| `/api/reviews/:part_key/reconcile` | POST | Pull fresh Informix data into Postgres; cache any new documents; return changelog |
| `/api/reviews/excuse/process` | POST | Admin processes excuse (send to CEO or action) |
| `/api/reviews/disqualify/process` | POST | Admin processes disqualification |
| `/api/reviews/send-to-ceo` | POST | Admin sends prepped record to CEO queue — blocks if CEO decision already exists |
| `/api/reviews/recall` | POST | Admin recalls a pending_ceo record — no confirmation required |
| `/api/reviews/send-back` | POST | CEO sends record back to admin |
| `/api/reviews/ceo/decide` | POST | CEO makes final determination |
| `/api/reviews/records/:part_no` | GET | Review history |
| `/api/reviews/ceo-state` | GET | CEO review state (live/maintenance) |
| `/api/reviews/ceo-state` | POST | Toggle CEO review state |
| `/api/reviews/pending` | GET | Pending counts per queue |

## TypeScript Types

```typescript
interface ReviewRecord {
  part_no: string;
  pool_no: string;
  part_key: string;
  fname: string | null;
  lname: string | null;
  review_type: 'excuse' | 'disqualify';
  status: 'pending_admin' | 'pending_ceo' | 'completed' | 'sent_back';
  admin_notes: string | null;
  submitted_at: string;
  data_stale: boolean;
  stale_detected_at: string | null;
  data_version: number;  // incremented on each reconcile; CEO page polls on this
}

interface ReviewDetail {
  record: ReviewRecord;
  participant: ParticipantDetail;
  questionnaire: QuestionnaireDetail | null;
  scanned_doc_url: string | null;
  history: ReviewHistoryEntry[];
}

interface VerifyResult {
  matches: boolean;
  field_diffs: FieldDiff[];         // fields where Informix differs from Postgres
  documents_added: string[];        // document identifiers in Informix not in document_cache
  documents_removed: string[];      // in cache but no longer in Informix part_image
}

interface FieldDiff {
  field: string;
  postgres_value: string | null;
  informix_value: string | null;
}

interface ReconcileResult {
  changelog: FieldDiff[];           // what changed (before vs after); empty if nothing changed
  documents_cached: number;         // count of newly fetched documents
}

interface SendToCeoResult {
  sent: boolean;
  blocked_by_decision: CeoDecisionSummary | null;  // non-null if CEO already decided
}

interface CeoDecisionSummary {
  decision: string;
  decided_at: string;
  decided_by: string;
  informix_sync_status: 'pending' | 'completed' | 'failed';
}

interface ReviewDecision {
  part_key: string;
  action: 'requalify' | 'disqualify' | 'permanent_excuse' | 'temporary_excuse' | 'send_to_ceo' | 'send_back';
  notes: string;
}

interface ReviewHistoryEntry {
  part_no: string;
  review_type: string;
  action: string;
  actor_email: string | null;
  notes: string | null;
  acted_at: string;
}

interface CeoReviewState {
  state: 'live' | 'maintenance';
}
```

## Chunks

### 5.1 Backend: Admin queue APIs

`GET /api/reviews/excuse/admin` and `GET /api/reviews/disqualify/admin` —
query Informix `review_record` table for records with status `P` (pending admin).
Include `data_stale` and `stale_detected_at` from `status_reviews` in the response
so the admin queue can surface stale indicators per record.

**Verify:** Returns seed review records. Count matches seed data.

### 5.2 Build admin excuse queue page

`/reviews/excuse` — table of pending excuse requests. Click a row to open
individual review. Count badge in navbar.

Each record fires an async verify against Informix on page load (non-blocking).
Records show a per-record status indicator:
- Gray "checking..." while verify is in flight
- Green checkmark if data matches
- Yellow "Updates available" badge if `VerifyResult.matches === false`

Clicking "Updates available" navigates to the individual review where admin
can reconcile.

**Verify:** Records display with correct data from seed. Async verify fires
and updates indicators without blocking the page load.

### 5.3 Build admin disqualification queue page

`/reviews/disqualify` — same layout as excuse queue including async verify
and per-record indicators.

**Verify:** Disqualification records display.

### 5.3a Admin review screen stale banner

When any records in the admin queue have `data_stale = true` (flagged by the
stale cron — see Phase 7), a banner appears at the top of the admin review
screen:

> "2 records sent to CEO have updated data — review required"

Each flagged record is linked from the banner. The banner is dismissed
automatically when no stale records remain.

This is distinct from the per-record async verify on the queue page — the
banner reflects records already sent to CEO where drift was detected by the
background cron, not records still in the admin queue.

**Verify:** Banner appears when `data_stale` records exist. Disappears after
recall + reconcile.

### 5.4 Backend: Individual review data API

`GET /api/reviews/:part_key` — returns participant data from Informix plus
questionnaire responses and any stored documents.

**Verify:** Returns correct data for seed part_keys.

### 5.5 Build individual review page (admin view)

`/reviews/excuse/:part_key` and `/reviews/disqualify/:part_key`:
- Participant data panel (name, pool, address, demographics)
- Questionnaire responses
- Document viewer (scanned doc if available)
- Admin notes field
- **"Send to CEO"** button
- **"Reconcile"** button — visible when verify has returned differences

**Reconcile flow:**
1. Admin clicks "Reconcile"
2. `POST /api/reviews/:part_key/reconcile` — overwrites Postgres with Informix data, caches new documents
3. Page shows a changelog summary: "Name updated · 1 new document added"
4. Changelog is informational only — admin reviews it and decides whether to proceed

**Verify:** Reconcile updates Postgres and shows changelog. New documents appear
in the document viewer.

### 5.6 Backend: Verify and Reconcile APIs

`GET /api/reviews/:part_key/verify`:
- Query Informix for all displayed fields + `part_image` document identifiers
- Compare against Postgres `status_reviews` / `participant` data and `document_cache`
- Return `VerifyResult` — field diffs and document deltas
- Non-blocking from the frontend perspective; fires async per record

`POST /api/reviews/:part_key/reconcile`:
- Capture current Postgres state (for changelog)
- Overwrite Postgres participant fields with Informix values
- Query `part_image` for document identifiers not in `document_cache`; trigger WebDAV fetch for each
- Compute and return `ReconcileResult` changelog
- Reconcile always writes regardless of record workflow status (`pending_admin`, `pending_ceo`, etc.)
- Increment `data_version` on `status_reviews`

**Verify:** Reconcile on a seed record with known differences updates Postgres
and returns correct changelog.

### 5.7 Backend: Send to CEO (with blocking check)

`POST /api/reviews/send-to-ceo`:
- Before any write, check whether a CEO decision already exists for this `part_key`
  (`status_reviews.status IN ('completed', 'sent_back')` or a decision row in `review_history`)
- If decision exists: return `{ sent: false, blocked_by_decision: CeoDecisionSummary }`
  including the Informix sync status for that decision
- If no decision: proceed with send — update `status_reviews.status` to `pending_ceo`,
  record action in `review_history`, update Informix `review_record.status` to `S`

**Verify:** Record disappears from admin queue, appears in CEO queue when no
prior decision exists. Returns blocked response with decision details when CEO
has already decided.

### 5.7a Frontend: Send to CEO blocking modal

When `send-to-ceo` returns `blocked_by_decision`:

Display a blocking modal:

> **CEO Decision Already Recorded**
> CEO decided: [Temporary Excuse] on [date] by [name]
> Informix sync: [Pending / Completed / Failed]
> Further processing for this participant must be handled manually.

Modal has a single close button. No re-send option. Admin handles out of band
(email, phone). Record remains in its current state.

**Verify:** Modal appears with correct decision details and sync status.

### 5.8 Backend: Recall

`POST /api/reviews/recall`:
- No confirmation required
- If `status_reviews.status = 'pending_ceo'`: flip to `pending_admin`, clear `data_stale` / `stale_detected_at`, queue `reopen_review_record` in `informix_sync_queue`
- If `status_reviews.status` is already `completed` or `sent_back` (CEO decided concurrently): do nothing, return current state — the re-send attempt will surface the blocking modal
- Log recall action in `review_history`

**Verify:** Record flips to `pending_admin` immediately. CEO queue reflects
change on next poll. Concurrent CEO decision is not overwritten.

### 5.9 Backend: CEO queue API

`GET /api/reviews/ceo` — returns only records with `status = 'pending_ceo'`.
Includes `data_version` per record for client-side change detection.
Access requires `ceo-review` group.

**Verify:** Only sent records visible. Non-CEO user gets 403.

### 5.10 Build CEO queue page

`/reviews/ceo` — streamlined combined scrollable page, all pending cases inline
with decision forms. No navbar to rest of app for CEO role.

**Polling:**
- Page polls `GET /api/reviews/ceo` every 30 seconds
- Records recalled since last load are removed from the page quietly
- If a record's `data_version` has incremented since the page loaded, a banner
  appears on that specific record:
  > "This record was updated since it was loaded — [field summary e.g. 'address updated · 1 document added']"
- The banner is informational only — CEO is never blocked from submitting a decision
- Decision buttons are never disabled by poll results

**Verify:** Recalled records disappear on next poll. Updated records show
per-record banner with change summary. CEO can still submit decision on a
record showing the update banner.

### 5.11 Backend: CEO decision processing

`POST /api/reviews/ceo/decide` — **async write pattern, no ODBC on the hot path.**

All writes are a single PostgreSQL transaction:

```
BEGIN;
  UPDATE status_reviews SET status = 'completed'/'sent_back',
    decision = ..., ceo_notes = ..., decided_at = now(), decided_by = ...
    WHERE part_key = :part_key AND status = 'pending_ceo';  -- guard clause
  INSERT INTO review_history (action, actor_sub, notes, ...);
  INSERT INTO informix_sync_queue (operation, payload) VALUES
    ('update_pool_member_status', { part_no, pool_no, new_status }),
    ('close_review_record',       { part_no, pool_no, new_status });
COMMIT;
```

**Idempotency:** If `status` is already `completed` or `sent_back`, return
existing state as 200. Handles the case where the transaction committed but
the response was lost.

CEO is never blocked — decision commits regardless of whether the record
was reconciled after being sent, or whether the poll banner was shown.

**Verify:** All PostgreSQL tables updated correctly. Two sync queue rows inserted.
Re-submitting the same decision returns 200 with existing state.

### 5.12 Build review records history

`/reviews/records/:part_no` — complete review history for a participant,
including reconcile events, stale detections, recall attempts, and decisions.

**Verify:** Shows all actions from seed data and any test actions.

### 5.13 CEO review state toggle

Admin can toggle CEO review to maintenance mode:
- `GET/POST /api/reviews/ceo-state`
- Reads/writes `app_config.ceo_review_state`
- CEO queue shows maintenance message when in maintenance mode

**Verify:** Toggle updates state. CEO queue shows maintenance message.

### 5.14 Write E2E tests

1. Admin excuse queue loads with seed records; async verify fires and updates indicators
2. Admin disqualify queue loads
3. Individual review — all panels render, docs load
4. Reconcile — changelog displays, Postgres updated, new documents visible
5. Send to CEO — record moves to CEO queue when no prior decision
6. Send to CEO blocked — modal appears with decision details when CEO already decided
7. Recall — record returns to admin queue immediately, no confirmation
8. CEO queue (ceo-review role) — only prepped records visible, role gate works
9. CEO poll — recalled records disappear; updated records show per-record banner
10. CEO decision — each action updates DB correctly; CEO never blocked
11. Send back to admin — record returns to admin queue
12. Review history — full audit trail including reconcile and stale events
13. CEO maintenance mode — queue blocked, admin sees toggle
14. Stale banner — appears on admin review screen when pending_ceo records are flagged

**Verify:** Full workflow: admin preps → sends to CEO → CEO decides.
Reconcile and stale flows verified independently.

## Implementation Status

### Backend (Axum/Rust)
- [x] 5.1 Admin excuse + disqualify queue APIs
- [x] 5.4 Individual review data API
- [x] 5.7 Send-to-CEO API — **needs update: add blocking check for prior CEO decision**
- [x] 5.9 CEO queue API — **needs update: include `data_version` in response**
- [x] 5.11 CEO decision API — PG-only, sync queue for Informix
- [x] 5.12 Review history API
- [x] 5.13 CEO state toggle
- [x] Pending counts API
- [x] Document handling (`crates/server/src/documents.rs`)
- [x] Sync status API
- [ ] `GET /api/reviews/:part_key/verify` — not built
- [ ] `POST /api/reviews/:part_key/reconcile` — not built
- [ ] `POST /api/reviews/recall` — not built
- [ ] Send-to-CEO blocking check — not built
- [ ] `data_version` on CEO queue response — not built

### Frontend (SvelteKit)
- [x] Admin excuse queue (`/reviews/excuse`)
- [x] Admin disqualify queue (`/reviews/disqualify`)
- [x] Individual review + Send-to-CEO (`/reviews/excuse/[part_key]`, `/reviews/disqualify/[part_key]`)
- [x] CEO combined scrollable page (`/reviews/ceo`)
- [x] Review history page (`/reviews/records/[part_no]`)
- [x] CEO state toggle
- [x] Sync status admin page (`/reviews/sync`)
- [ ] Per-record async verify indicators on admin queue pages — not built
- [ ] Reconcile button and changelog display on individual review — not built
- [ ] Send-to-CEO blocking modal — not built
- [ ] Recall button — not built
- [ ] Stale banner on admin review screen — not built
- [ ] CEO poll with per-record update banner — not built

### Testing (5.14)
- [x] `tests/reviews.test.ts` — existing tests
- [ ] Verify, reconcile, recall, blocking modal, CEO poll banner tests — not written

## Notes

- **Reconcile is always safe to run** — it writes regardless of workflow status and never
  blocks any subsequent action. It is independent of the send-to-CEO gate.
- **CEO is never blocked** — poll results and update banners are informational only.
  The decision guard clause in `ceo_decide_handler` protects against double-write,
  not against stale data.
- **`data_version`** on `status_reviews` is incremented by each reconcile. The CEO
  poll compares the version seen at page load against the current API response to
  determine whether to show the per-record update banner.
- **Document lifecycle**: see Phase 7 for document cache cleanup on sync completion.

## Exit Criteria

- [x] Admin queues display correct records from Informix
- [x] Send-to-CEO moves records correctly
- [x] CEO queue shows only prepped records; role gate enforced
- [x] CEO decisions update all tables correctly
- [x] Send back to admin returns record
- [x] Review history shows complete audit trail
- [x] CEO maintenance mode works
- [x] Document metadata returned for participants with `part_image` records
- [x] Sync status page shows cross-system health
- [ ] Async verify fires per record on admin queue pages
- [ ] Reconcile updates Postgres and shows changelog
- [ ] Recall works without confirmation
- [ ] Send-to-CEO blocking modal appears when CEO has already decided
- [ ] CEO poll removes recalled records and shows per-record update banners
- [ ] Stale banner appears on admin review screen
- [ ] All Puppeteer tests pass
- [ ] Developer has verified full workflow locally