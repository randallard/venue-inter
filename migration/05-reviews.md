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
| `/api/reviews/excuse/process` | POST | Admin processes excuse (send to CEO or action) |
| `/api/reviews/disqualify/process` | POST | Admin processes disqualification |
| `/api/reviews/send-to-ceo` | POST | Admin sends prepped record to CEO queue |
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
}

interface ReviewDetail {
  record: ReviewRecord;
  participant: ParticipantDetail;
  questionnaire: QuestionnaireDetail | null;
  scanned_doc_url: string | null;
  history: ReviewHistoryEntry[];
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

**Verify:** Returns seed review records. Count matches seed data.

### 5.2 Build admin excuse queue page

`/reviews/excuse` — table of pending excuse requests. Click a row to open
individual review. Count badge in navbar.

**Verify:** Records display with correct data from seed.

### 5.3 Build admin disqualification queue page

`/reviews/disqualify` — same layout as excuse queue.

**Verify:** Disqualification records display.

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
- **"Send to CEO"** button (becomes active once admin has reviewed)

**Verify:** All data panels render. Send to CEO moves record to CEO queue.

### 5.6 Backend: Send to CEO

`POST /api/reviews/send-to-ceo` — updates `status_reviews.status` to
`pending_ceo`, records action in `review_history`, updates Informix
`review_record.status` to `S`.

**Verify:** Record disappears from admin queue, appears in CEO queue.

### 5.7 Backend: CEO queue API

`GET /api/reviews/ceo` — returns only records with `status = 'pending_ceo'`.
Access requires `ceo-review` group.

**Verify:** Only sent records visible. Non-CEO user gets 403.

### 5.8 Build CEO queue page

`/reviews/ceo` — streamlined list, no navbar to rest of app for CEO role.
Shows only: participant name, review type, show/pool, date sent.
Click a row to open CEO decision view.

**Verify:** CEO user sees only this page. Non-CEO cannot access it.

### 5.9 Build CEO decision view

`/reviews/ceo/:part_key` — focused single-case view:
- Participant data (read-only)
- Questionnaire and scanned document
- Admin notes (read-only)
- **CEO decision buttons:**
  - Re-qualify (status 2)
  - Disqualify (status 6)
  - Permanent Excuse (status 5)
  - Temporary Excuse (status 7)
  - Send Back to Admin
- CEO notes field (required for all decisions)

No navigation to other parts of the system from this view.

**Verify:** Each decision button executes correctly, updates pool_member status,
records in review_history.

### 5.10 Backend: CEO decision processing

`POST /api/reviews/ceo/decide` — **async write pattern, no ODBC on the hot path.**

All writes are a single PostgreSQL transaction (fast, local):

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

Informix is updated by the sync cron (Phase 7), not inline.

**Idempotency:** If `status_reviews.status` is already `completed` or `sent_back`
when the request arrives, return the existing state as a 200 (not an error).
This handles the case where the transaction committed but the response was lost.

**Verify:** All PostgreSQL tables updated correctly. Two sync queue rows inserted.
Re-submitting the same decision returns 200 with existing state.

#### Durability guarantee for CEO decision

The decision is durable the moment the PostgreSQL transaction commits — PostgreSQL
WAL ensures it survives crashes and restarts. The sequence is:

1. CEO clicks → browser disables decision buttons immediately (prevents double-submit)
2. POST reaches Axum → transaction opens
3. Transaction commits → decision is **durable and irrecoverable**
4. Axum returns 200 → browser animates record out of queue

**Failure cases, none result in data loss:**

| Failure point | What happens | CEO sees |
|---|---|---|
| Network drops before POST reaches server | No write occurred | Timeout; buttons re-enable, can retry |
| Server crash mid-transaction | PG rolls back atomically | Same as above |
| Transaction commits, response lost | Decision is saved | Timeout; re-fetch detects `completed`, browser treats as success |
| CEO double-clicks | Guard clause in UPDATE prevents double-write | Second request returns existing state |

**Browser behaviour on error:**
- On 5xx or timeout: re-fetch the record status before showing an error
- If record is now `completed` or `sent_back`: treat as success (record disappears cleanly)
- If still `pending_ceo`: re-enable buttons with non-intrusive inline error on the record
- Do not interrupt flow if CEO has already navigated to the next record

This matches the current system's UX contract: record disappears on success,
error is surfaced inline on the record rather than as a modal that blocks flow.

**Verify:** All PostgreSQL tables updated correctly. Two sync queue rows inserted.
Re-submitting the same decision returns 200 with existing state.

### 5.11 Build review records history

`/reviews/records/:part_no` — complete review history for a participant.

**Verify:** Shows all actions from seed data and any test actions.

### 5.12 CEO review state toggle

Admin can toggle CEO review to maintenance mode:
- `GET/POST /api/reviews/ceo-state`
- Reads/writes `app_config.ceo_review_state`
- CEO queue shows maintenance message when in maintenance mode

**Verify:** Toggle updates state. CEO queue shows maintenance message.

### 5.13 Write E2E tests

1. Admin excuse queue loads with seed records
2. Admin disqualify queue loads
3. Individual review — all panels render, docs load
4. Send to CEO — record moves to CEO queue
5. CEO queue (ceo-review role) — only prepped records visible, role gate works
6. CEO decision — each action updates DB correctly
7. Send back to admin — record returns to admin queue
8. Review history — full audit trail visible
9. CEO maintenance mode — queue blocked, admin sees toggle

**Verify:** Full workflow: admin preps → sends to CEO → CEO decides.

## Implementation Status

### Backend (Axum/Rust)
- [x] 5.1 Admin excuse + disqualify queue APIs (`admin_excuse_queue_handler`, `admin_disqualify_queue_handler`)
- [x] 5.4 Individual review data API (`review_detail_handler`)
- [x] 5.6 Send-to-CEO API (`send_to_ceo_handler`)
- [x] 5.7 CEO queue API (`ceo_queue_handler`)
- [x] 5.10 CEO decision API (`ceo_decide_handler`) — PG-only, sync queue for Informix
- [x] 5.11 Review history API (`review_history_handler`)
- [x] 5.12 CEO state toggle (`get_ceo_state_handler`, `set_ceo_state_handler`)
- [x] Pending counts API (`pending_counts_handler`)
- [ ] Document viewer endpoint (`/api/reviews/:part_key/document`) — not built

### Frontend (SvelteKit)
- [x] 5.2 Admin excuse queue (`/reviews/excuse`)
- [x] 5.3 Admin disqualify queue (`/reviews/disqualify`)
- [x] 5.5 Individual review + Send-to-CEO (`/reviews/excuse/[part_key]`, `/reviews/disqualify/[part_key]`)
- [x] 5.8 CEO queue page (`/reviews/ceo`)
- [x] 5.9 CEO decision view (`/reviews/ceo/[part_key]`)
- [x] 5.11 Review history page (`/reviews/records/[part_no]`)
- [x] 5.12 CEO state toggle (in CEO queue page)

### Testing (5.13)
- [x] Puppeteer E2E tests written (`tests/reviews.test.ts`); run with `pnpm test:e2e`

## Exit Criteria

- [x] Admin queues display correct records from Informix
- [x] Send-to-CEO moves records correctly
- [x] CEO queue shows only prepped records; role gate enforced
- [x] CEO decisions update all tables correctly
- [x] Send back to admin returns record
- [x] Review history shows complete audit trail
- [x] CEO maintenance mode works
- [ ] All Puppeteer tests pass (run `pnpm test:e2e` after verifying locally)
- [ ] Developer has verified full workflow
