# Phase 3: Pool Management

## Goal

Build pool management covering active pools, participant lookup, participant draw,
and seating charts.

## Routes

| Route | Description |
|---|---|
| `/pools` | Active pools listing |
| `/pools/draw` | Participant draw page |
| `/pools/seating` | Seating chart generation |
| `/pools/lookup` | Participant data lookup |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/pools/active` | GET | Pools with `ret_date >= TODAY` |
| `/api/pools/:pool_no/members` | GET | Members for a pool |
| `/api/participants/:part_no` | GET | Participant lookup |
| `/api/pools/draw` | POST | Execute a random participant draw |
| `/api/pools/draw/export` | POST | Export draw results as Excel |
| `/api/pools/seating` | POST | Generate randomized seating chart |
| `/api/pools/seating/download` | GET | Download generated seating files |

## TypeScript Types

```typescript
interface Pool {
  pool_no: number;
  show_no: number | null;
  ret_date: string | null;
  div_code: string | null;
  office: string | null;
  capacity: number | null;
  member_count: number;
}

interface PoolMember {
  pm_id: number;
  pool_no: number;
  part_no: number;
  fname: string | null;
  lname: string | null;
  status: number;
  rand_nbr: number | null;
  responded: string | null;
}

interface ParticipantDetail {
  part_no: number;
  fname: string | null;
  lname: string | null;
  addr: string | null;
  city: string | null;
  state: string | null;
  zip: string | null;
  dob: string | null;
  gender: string | null;
  race_code: string | null;
  pool_history: PoolHistoryEntry[];
}

interface DrawRequest {
  pool_no: number;
  div_code: string;
  count: number;
}

interface DrawResult {
  participants: DrawnParticipant[];
  total_drawn: number;
}

interface DrawnParticipant {
  rand_nbr: number;
  part_no: number;
  fname: string | null;
  lname: string | null;
}
```

## Chunks

### 3.1 Backend: Active pools API

`GET /api/pools/active` — query pools with `ret_date >= TODAY` joined with
member count.

**Verify:** Returns the two seed pools (Theater + Sports).

### 3.2 Build active pools page

`/pools` — table of active pools with pool number, show date, division, capacity,
member count. Clicking a row opens pool detail.

**Verify:** Two pools display with correct counts from seed data.

### 3.3 Backend: Pool members API

`GET /api/pools/:pool_no/members` — pool members joined with participant name.

**Verify:** Member count and status values match seed data.

### 3.4 Backend: Participant draw API

`POST /api/pools/draw` — assigns random numbers to eligible pool members,
returns sorted draw result.

**Verify:** Returns correct count. Random ordering differs across calls.

### 3.5 Build participant draw page

`/pools/draw` — select pool, enter draw count, execute draw, display results table
with export button.

**Verify:** Draw executes, results display. Export downloads a file.

### 3.6 Backend: Excel export

`POST /api/pools/draw/export` — returns Excel file of draw results.

**Verify:** Downloaded file opens correctly with correct data.

### 3.7 Build seating chart page

`/pools/seating` — select pool, generate randomized seating, download link.

**Verify:** Seating generates with randomized order.

### 3.8 Backend: Seating chart API

`POST /api/pools/seating` — randomize eligible members, generate downloadable file.

**Verify:** File contains correct randomized data.

### 3.9 Build participant lookup page

`/pools/lookup` — search by part_no, display participant data with pool history.

**Verify:** Returns correct participant. History rows display.

### 3.10 Backend: Participant lookup API

`GET /api/participants/:part_no` — returns participant record + pool history.

**Verify:** Matches seed data for all test part_nos.

### 3.11 Write E2E tests

1. Active pools page loads with two pools
2. Pool detail — member table renders with correct statuses
3. Participant draw — select pool, draw, results display
4. Export — file download triggers
5. Seating chart — generate and download
6. Participant lookup — enter part_no, record and history display

**Verify:** All tests pass.

## Implementation Status

### Backend
- [x] `GET /api/pools` — pools listing
- [x] `GET /api/pools/{pool_no}/members` — pool members
- [x] `GET /api/participants` — participants list (bulk; no individual lookup endpoint)
- [x] `GET /api/pool_staff`
- [ ] `POST /api/pools/draw` — not built
- [ ] `POST /api/pools/draw/export` — not built
- [ ] `POST /api/pools/seating` — not built
- [ ] `GET /api/participants/:part_no` — individual lookup not built

### Frontend
- [x] `/pools` — pools listing with member count
- [ ] `/pools/draw` — not built
- [ ] `/pools/seating` — not built
- [ ] `/pools/lookup` — not built

### Testing
- [ ] `pools.test.ts` — not written

## Exit Criteria

- [x] Active pools page shows correct pool list
- [ ] Draw produces correct randomized results
- [ ] Excel export works
- [ ] Participant lookup returns correct data
- [ ] All Puppeteer tests pass
- [ ] Developer has verified each workflow
