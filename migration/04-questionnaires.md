# Phase 4: Questionnaires

## Goal

Build eligibility questionnaire generation, distribution tracking, and upload
for drawn participants. Each show can have a tailored set of eligibility questions.

## Routes

| Route | Description |
|---|---|
| `/pools/questionnaires` | Questionnaire management for active pools |
| `/pools/questionnaires/[pool_no]` | Questionnaire status for a specific pool |
| `/pools/questionnaires/[pool_no]/[part_no]` | Individual questionnaire view / upload |

## Backend APIs

| Endpoint | Method | Description |
|---|---|---|
| `/api/questionnaires/:pool_no` | GET | Questionnaire status for all pool members |
| `/api/questionnaires/:pool_no/:part_no` | GET | Individual questionnaire data |
| `/api/questionnaires/:pool_no/:part_no/upload` | POST | Upload completed questionnaire |
| `/api/questionnaires/generate` | POST | Generate questionnaire packet for pool |

## TypeScript Types

```typescript
interface QuestionnaireStatus {
  part_no: number;
  pool_no: number;
  fname: string | null;
  lname: string | null;
  status: number;
  responded: string;
  scan_code: string | null;
  has_questionnaire: boolean;
  submitted_at: string | null;
}

interface QuestionnaireDetail {
  part_no: number;
  pool_no: number;
  fname: string | null;
  lname: string | null;
  responses: QuestionnaireResponse[];
  scan_code: string | null;
  submitted_at: string | null;
}

interface QuestionnaireResponse {
  question_key: string;
  question_text: string;
  response: string | null;
}
```

## Chunks

### 4.1 Backend: Questionnaire status API

`GET /api/questionnaires/:pool_no` — list all pool members with questionnaire
completion status (based on `responded` flag and `scan_code` presence).

**Verify:** Status matches seed data (some responded, some not).

### 4.2 Build questionnaire management page

`/pools/questionnaires/:pool_no` — table of pool members with completion status.
Color-coded: green = complete, yellow = partial, red = missing.

**Verify:** Status badges render correctly.

### 4.3 Backend: Individual questionnaire API

`GET /api/questionnaires/:pool_no/:part_no` — participant detail plus any stored
questionnaire responses.

**Verify:** Returns correct participant data.

### 4.4 Backend: Questionnaire upload

`POST /api/questionnaires/:pool_no/:part_no/upload` — accept scanned document
or form data, store reference in Informix, update `responded` flag.

**Verify:** Upload completes, `responded` flag updates in DB.

### 4.5 Build individual questionnaire view

`/pools/questionnaires/:pool_no/:part_no` — show participant data, response
display, upload control. Confirmation before upload.

**Verify:** Upload flow completes, status badge updates.

### 4.6 Backend: Generate questionnaire packet

`POST /api/questionnaires/generate` — generate printable questionnaire PDF
for all non-responding members in a pool.

**Verify:** PDF generated with correct participant data.

### 4.7 Write E2E tests

1. Questionnaire list loads with correct statuses
2. Individual view displays participant data
3. Upload flow completes, status updates
4. Generate packet downloads PDF

**Verify:** All tests pass.

## Exit Criteria

- [ ] Questionnaire status page shows correct completion states
- [ ] Upload stores data and updates status
- [ ] PDF generation works
- [ ] All Puppeteer tests pass
- [ ] Developer has verified each workflow
