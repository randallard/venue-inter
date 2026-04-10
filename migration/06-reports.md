# Phase 6: Reports

## Goal

Build demographic reports, address verification, and participation history reports.

## Routes

| Route | Description |
|---|---|
| `/reports` | Reports landing page |
| `/reports/demographics` | Race / gender demographics by pool |
| `/reports/address-verification` | Address verification (NCOA) status |
| `/reports/participation-history` | Participation history summary |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/reports/demographics/:pool_no` | GET | Race/gender breakdown for a pool |
| `/api/reports/demographics/export` | POST | Export demographics as CSV/Excel |
| `/api/reports/address-verification` | GET | Participants with unverified addresses |
| `/api/reports/participation-history` | GET | Participation counts per participant |

## TypeScript Types

```typescript
interface DemographicsRow {
  race_code: string;
  gender: string;
  count: number;
  percentage: number;
}

interface DemographicsReport {
  pool_no: number;
  ret_date: string | null;
  total: number;
  breakdown: DemographicsRow[];
}

interface AddressVerificationRow {
  part_no: number;
  fname: string | null;
  lname: string | null;
  addr: string | null;
  city: string | null;
  state: string | null;
  zip: string | null;
  verification_status: string;
}

interface ParticipationHistoryRow {
  part_no: number;
  fname: string | null;
  lname: string | null;
  pool_count: number;
  last_pool_date: string | null;
  current_status: string;
}
```

## Chunks

### 6.1 Backend: Demographics report

`GET /api/reports/demographics/:pool_no` — aggregate race_code and gender
counts from `pool_member` joined with `participant`.

**Verify:** Counts match seed data for both pools.

### 6.2 Build demographics page

`/reports/demographics` — pool selector, then table + simple bar chart per
race/gender breakdown. Export button.

**Verify:** Correct counts display. Export downloads file.

### 6.3 Backend: Address verification report

`GET /api/reports/address-verification` — list participants with incomplete
or mismatched address data.

**Verify:** Returns relevant rows from seed data.

### 6.4 Build address verification page

`/reports/address-verification` — table of flagged participants with address
fields. Filter by pool or status.

**Verify:** Correct records display.

### 6.5 Backend: Participation history report

`GET /api/reports/participation-history` — count of pool appearances and
last pool date per participant, joined from `part_history`.

**Verify:** Counts match seed data.

### 6.6 Build participation history page

`/reports/participation-history` — sortable table with participation counts.
Click a row to open participant detail.

**Verify:** Counts display correctly. Row click navigates to lookup page.

### 6.7 Write E2E tests

1. Demographics — pool selector loads, counts correct, export works
2. Address verification — flagged records display
3. Participation history — counts correct, row navigation works

**Verify:** All tests pass.

## Implementation Status

- [x] `/reports` — placeholder landing page only
- [ ] Demographics report — not built
- [ ] Address verification report — not built
- [ ] Participation history report — not built
- [ ] No backend API endpoints for any report
- [ ] `reports.test.ts` — not written

## Exit Criteria

- [ ] All three reports display correct data
- [ ] Exports work
- [ ] All Puppeteer tests pass
- [ ] Developer has verified each report
