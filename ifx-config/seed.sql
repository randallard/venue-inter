-- ============================================================================
-- VenueInter — Venue Audience Management System
-- Legacy Informix schema for tracking participants, shows, pools, draws,
-- eligibility questionnaires, and review records.
--
-- Corporate loads participant data every two years. The app manages pool
-- formation, draws, eligibility verification, and status reviews.
-- ============================================================================

CREATE DATABASE venueinter WITH LOG;

DATABASE venueinter;

-- --------------------------------------------------------------------------
-- Lookup tables
-- --------------------------------------------------------------------------

CREATE TABLE show_type (
    st_code         CHAR(8)      PRIMARY KEY,
    st_description  VARCHAR(100) NOT NULL
);

INSERT INTO show_type VALUES ('CONCERT', 'Concert');
INSERT INTO show_type VALUES ('THEATER', 'Theater / Live Performance');
INSERT INTO show_type VALUES ('SPORTS',  'Sporting Event');
INSERT INTO show_type VALUES ('SPEVENT', 'Special Event');
INSERT INTO show_type VALUES ('PREVIEW', 'Preview Screening');

CREATE TABLE venue (
    venue_code  CHAR(4)      PRIMARY KEY,
    venue_name  VARCHAR(100) NOT NULL,
    venue_addr  VARCHAR(200),
    venue_city  VARCHAR(50),
    venue_state CHAR(2),
    venue_zip   CHAR(10),
    active      CHAR(1)      DEFAULT 'Y'
);

INSERT INTO venue VALUES ('ARNA', 'City Arena',               '500 Arena Blvd',     'Springfield', 'IL', '62701', 'Y');
INSERT INTO venue VALUES ('THTR', 'Downtown Theater',         '120 Main St',        'Springfield', 'IL', '62701', 'Y');
INSERT INTO venue VALUES ('STDM', 'Lakefront Stadium',        '1 Stadium Dr',       'Springfield', 'IL', '62702', 'Y');
INSERT INTO venue VALUES ('CNVN', 'Convention Center',        '300 Convention Way', 'Springfield', 'IL', '62703', 'Y');

-- --------------------------------------------------------------------------
-- Shows (upcoming events needing audience members)
-- --------------------------------------------------------------------------

CREATE TABLE show (
    show_no         SERIAL       PRIMARY KEY,
    show_name       VARCHAR(150) NOT NULL,
    show_date       DATE         NOT NULL,
    show_type_code  CHAR(8),
    venue_code      CHAR(4),
    capacity        SMALLINT,
    status          CHAR(1)      DEFAULT 'A'   -- A=active, C=closed, X=cancelled
);

-- Past show
INSERT INTO show VALUES (0, 'Spring Music Festival',          '02/15/2026', 'CONCERT', 'ARNA', 150, 'C');
-- Upcoming shows (pools active)
INSERT INTO show VALUES (0, 'Regional Theater Opening Night', '05/10/2026', 'THEATER', 'THTR',  80, 'A');
INSERT INTO show VALUES (0, 'All-Star Weekend Game',          '05/24/2026', 'SPORTS',  'STDM', 200, 'A');
-- Future show (no pool yet)
INSERT INTO show VALUES (0, 'Summer Concert Series',          '07/12/2026', 'CONCERT', 'ARNA', 175, 'A');

-- --------------------------------------------------------------------------
-- Participant (corporate-loaded every two years)
-- active: A=active on list, I=inactive
-- --------------------------------------------------------------------------

CREATE TABLE participant (
    part_no     SERIAL       PRIMARY KEY,
    lname       VARCHAR(50)  NOT NULL,
    fname       VARCHAR(50)  NOT NULL,
    mi          CHAR(1),
    addr        VARCHAR(100),
    city        VARCHAR(50),
    state       CHAR(2),
    zip         CHAR(10),
    phone       VARCHAR(20),
    email       VARCHAR(100),
    dob         DATE,
    race_code   CHAR(2),
    gender      CHAR(1),
    active      CHAR(1)      DEFAULT 'A',
    date_added  DATE         DEFAULT TODAY
);

INSERT INTO participant VALUES (0, 'Anderson',  'James',    'R', '421 Oak St',         'Springfield', 'IL', '62701', '555-0201', 'j.anderson@example.com',  '04/12/1978', 'W', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Martinez',  'Sofia',    NULL,'88 Pine Ave',         'Springfield', 'IL', '62702', '555-0202', 's.martinez@example.com',  '07/22/1985', 'H', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Johnson',   'Marcus',   'T', '15 Elm Rd',           'Springfield', 'IL', '62701', '555-0203', 'm.johnson@example.com',   '11/05/1990', 'B', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Okafor',    'Adaeze',   NULL,'302 Maple Dr',         'Springfield', 'IL', '62703', '555-0204', 'a.okafor@example.com',    '03/18/1982', 'B', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Chen',      'Wei',      NULL,'57 Willow Ln',         'Springfield', 'IL', '62701', '555-0205', 'w.chen@example.com',      '09/30/1975', 'A', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Williams',  'Jasmine',  'L', '1090 Cedar Blvd',      'Springfield', 'IL', '62702', '555-0206', 'j.williams@example.com',  '06/14/1988', 'B', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Petrov',    'Dmitri',   'S', '44 Birch Ct',          'Springfield', 'IL', '62701', '555-0207', 'd.petrov@example.com',    '12/01/1980', 'W', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Nguyen',    'Linh',     NULL,'720 Spruce St',         'Springfield', 'IL', '62703', '555-0208', 'l.nguyen@example.com',    '02/28/1993', 'A', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Thompson',  'Robert',   NULL,'33 Ash Way',            'Springfield', 'IL', '62701', '555-0209', 'r.thompson@example.com',  '08/17/1972', 'W', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Alvarez',   'Maria',    'E', '219 Poplar Ave',        'Springfield', 'IL', '62702', '555-0210', 'm.alvarez@example.com',   '05/05/1987', 'H', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Park',      'Jin-ho',   NULL,'614 Walnut St',          'Springfield', 'IL', '62701', '555-0211', 'j.park@example.com',      '01/20/1995', 'A', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Davis',     'Sarah',    'K', '88 Chestnut Rd',         'Springfield', 'IL', '62703', '555-0212', 's.davis@example.com',     '10/09/1983', 'W', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Eriksson',  'Lars',     NULL,'7 Hickory Pl',           'Springfield', 'IL', '62701', '555-0213', 'l.eriksson@example.com',  '07/04/1976', 'W', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Abadi',     'Tariq',    'M', '450 Sycamore Ave',       'Springfield', 'IL', '62702', '555-0214', 't.abadi@example.com',     '04/25/1998', 'B', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Kim',       'Soo-yeon', NULL,'299 Magnolia Dr',         'Springfield', 'IL', '62701', '555-0215', 's.kim@example.com',       '09/13/1991', 'A', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Romano',    'Anthony',  NULL,'182 Hawthorn Blvd',      'Springfield', 'IL', '62703', '555-0216', 'a.romano@example.com',    '03/07/1969', 'W', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Walsh',     'Catherine','M', '55 Locust St',            'Springfield', 'IL', '62701', '555-0217', 'c.walsh@example.com',     '11/22/1984', 'W', 'F', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Baker',     'James',    NULL,'910 Mulberry Ave',        'Springfield', 'IL', '62702', '555-0218', 'j.baker@example.com',     '06/30/1979', 'W', 'M', 'I', '03/01/2024');
INSERT INTO participant VALUES (0, 'Nakamura',  'Hiro',     NULL,'125 Cypress Way',         'Springfield', 'IL', '62701', '555-0219', 'h.nakamura@example.com',  '02/14/1996', 'A', 'M', 'A', '03/01/2024');
INSERT INTO participant VALUES (0, 'Robinson',  'Diane',    'C', '660 Aspen Ct',            'Springfield', 'IL', '62703', '555-0220', 'd.robinson@example.com',  '08/08/1981', 'B', 'F', 'A', '03/01/2024');

-- --------------------------------------------------------------------------
-- Pool (draw group for a specific show)
-- --------------------------------------------------------------------------

CREATE TABLE pool (
    pool_no     SERIAL    PRIMARY KEY,
    show_no     INTEGER   NOT NULL,
    ret_date    DATE      NOT NULL,   -- date participants report (matches show_date)
    div_code    CHAR(8),              -- show type division code
    office      CHAR(1),             -- venue office identifier
    capacity    SMALLINT             -- target draw size
);

-- Pool for Theater Opening Night (show_no=2, ret_date=05/10/2026)
INSERT INTO pool VALUES (0, 2, '05/10/2026', 'THEATER', 'T', 40);
-- Pool for All-Star Weekend Game (show_no=3, ret_date=05/24/2026)
INSERT INTO pool VALUES (0, 3, '05/24/2026', 'SPORTS',  'S', 80);
-- Pool with a bad div_code (not in show_type) — for dashboard bad-code testing
INSERT INTO pool VALUES (0, 4, '07/01/2026', 'XBAD',   'A', 50);

-- --------------------------------------------------------------------------
-- Pool member (participant status within a pool)
-- Status codes (mirror UJMS):
--   1 = in pool / summoned
--   2 = qualified / selected
--   5 = permanently excused
--   6 = disqualified
--   7 = temporarily excused
-- --------------------------------------------------------------------------

CREATE TABLE pool_member (
    pm_id       SERIAL    PRIMARY KEY,
    pool_no     INTEGER   NOT NULL,
    part_no     INTEGER   NOT NULL,
    status      SMALLINT  NOT NULL DEFAULT 1,
    rand_nbr    INTEGER,
    responded   CHAR(1)   DEFAULT 'N',
    scan_code   VARCHAR(50)
);

-- Theater pool (pool_no=1) — 12 members
INSERT INTO pool_member VALUES (0, 1,  1,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 1,  2,  1, NULL, 'Y', 'TH2026-002');
INSERT INTO pool_member VALUES (0, 1,  3,  2, NULL, 'Y', 'TH2026-003');
INSERT INTO pool_member VALUES (0, 1,  4,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 1,  5,  1, NULL, 'Y', 'TH2026-005');
INSERT INTO pool_member VALUES (0, 1,  7,  7, NULL, 'Y', 'TH2026-007');  -- temp excused
INSERT INTO pool_member VALUES (0, 1,  9,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 1, 10,  1, NULL, 'Y', 'TH2026-010');
INSERT INTO pool_member VALUES (0, 1, 11,  6, NULL, 'Y', 'TH2026-011');  -- disqualified
INSERT INTO pool_member VALUES (0, 1, 13,  2, NULL, 'Y', 'TH2026-013');
INSERT INTO pool_member VALUES (0, 1, 15,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 1, 17,  5, NULL, 'Y', 'TH2026-017');  -- perm excused

-- Baker (part_no=18, active='I') in theater pool — for portal lockout testing
INSERT INTO pool_member VALUES (0, 1, 18, 1, NULL, 'N', NULL);

-- Sports pool (pool_no=2) — 15 members
INSERT INTO pool_member VALUES (0, 2,  1,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2,  3,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2,  5,  1, NULL, 'Y', 'SP2026-005');
INSERT INTO pool_member VALUES (0, 2,  6,  2, NULL, 'Y', 'SP2026-006');
INSERT INTO pool_member VALUES (0, 2,  8,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2,  9,  1, NULL, 'Y', 'SP2026-009');
INSERT INTO pool_member VALUES (0, 2, 11,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2, 12,  1, NULL, 'Y', 'SP2026-012');
INSERT INTO pool_member VALUES (0, 2, 14,  7, NULL, 'Y', 'SP2026-014');  -- temp excused
INSERT INTO pool_member VALUES (0, 2, 15,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2, 16,  1, NULL, 'Y', 'SP2026-016');
INSERT INTO pool_member VALUES (0, 2, 19,  1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 2, 20,  6, NULL, 'Y', 'SP2026-020');  -- disqualified
INSERT INTO pool_member VALUES (0, 2,  2,  1, NULL, 'Y', 'SP2026-002');
INSERT INTO pool_member VALUES (0, 2,  4,  2, NULL, 'Y', 'SP2026-004');

-- --------------------------------------------------------------------------
-- Participant history (status change audit trail)
-- --------------------------------------------------------------------------

CREATE TABLE part_history (
    ph_id       SERIAL       PRIMARY KEY,
    part_no     INTEGER      NOT NULL,
    pool_no     INTEGER      NOT NULL,
    old_status  SMALLINT,
    new_status  SMALLINT     NOT NULL,
    action      VARCHAR(50)  NOT NULL,
    actor       VARCHAR(50),
    acted_date  DATE         DEFAULT TODAY
);

INSERT INTO part_history VALUES (0,  7, 1, 1, 7, 'temp_excuse',  'admin', '04/01/2026');
INSERT INTO part_history VALUES (0, 17, 1, 1, 5, 'perm_excuse',  'admin', '04/02/2026');
INSERT INTO part_history VALUES (0, 11, 1, 1, 6, 'disqualify',   'admin', '04/03/2026');
INSERT INTO part_history VALUES (0, 14, 2, 1, 7, 'temp_excuse',  'admin', '04/05/2026');
INSERT INTO part_history VALUES (0, 20, 2, 1, 6, 'disqualify',   'admin', '04/06/2026');

-- --------------------------------------------------------------------------
-- Review record (pending excuse / disqualification reviews)
-- status: P=pending admin, S=sent to CEO, C=completed
-- --------------------------------------------------------------------------

CREATE TABLE review_record (
    rr_id           SERIAL       PRIMARY KEY,
    part_no         INTEGER      NOT NULL,
    pool_no         INTEGER      NOT NULL,
    review_type     CHAR(10)     NOT NULL,   -- 'excuse' or 'disqualify'
    status          CHAR(1)      DEFAULT 'P',
    admin_notes     VARCHAR(500),
    submitted_date  DATE         DEFAULT TODAY
);

-- Bad-code pool (pool_no=3) — 3 members for show code testing
INSERT INTO pool_member VALUES (0, 3,  6, 1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 3, 10, 1, NULL, 'N', NULL);
INSERT INTO pool_member VALUES (0, 3, 16, 1, NULL, 'N', NULL);

INSERT INTO review_record VALUES (0,  7, 1, 'excuse',     'P', 'Participant reports scheduling conflict with family event',  '04/01/2026');
INSERT INTO review_record VALUES (0, 17, 1, 'excuse',     'S', 'Permanent medical exemption — documentation verified',      '04/02/2026');
INSERT INTO review_record VALUES (0, 11, 1, 'disqualify', 'P', 'Failed eligibility check — prior disqualification on record','04/03/2026');
INSERT INTO review_record VALUES (0, 14, 2, 'excuse',     'P', 'Work conflict — requesting temporary deferral',              '04/05/2026');
INSERT INTO review_record VALUES (0, 20, 2, 'disqualify', 'S', 'Address verification failed — outside service area',         '04/06/2026');

-- --------------------------------------------------------------------------
-- Staff codes (lookup for pool session staff by type)
-- --------------------------------------------------------------------------

CREATE TABLE staff_codes (
    sc_type         VARCHAR(20)  NOT NULL,
    sc_code         VARCHAR(20)  NOT NULL,
    sc_translation  VARCHAR(255) NOT NULL,
    PRIMARY KEY (sc_type, sc_code)
);

INSERT INTO staff_codes VALUES ('coordinator', 'TRS', 'Torres, Rosa');
INSERT INTO staff_codes VALUES ('coordinator', 'CHN', 'Chang, David');
INSERT INTO staff_codes VALUES ('coordinator', 'PAR', 'Parsons, Kelly');
INSERT INTO staff_codes VALUES ('usher',       'GRN', 'Green, Marcus');
INSERT INTO staff_codes VALUES ('usher',       'SLV', 'Silva, Ana');
INSERT INTO staff_codes VALUES ('usher',       'OBR', 'O''Brien, Patrick');
INSERT INTO staff_codes VALUES ('security',    'HAR', 'Harris, Denise');
INSERT INTO staff_codes VALUES ('security',    'YNG', 'Young, Carl');

-- --------------------------------------------------------------------------
-- Session resources (staff assigned to future pool sessions)
-- --------------------------------------------------------------------------

CREATE TABLE session_resources (
    sr_id              SERIAL       PRIMARY KEY,
    sr_seqno           INTEGER,
    sr_pool_no         INTEGER      NOT NULL,
    sr_name            VARCHAR(255) NOT NULL,
    sr_type            VARCHAR(128) NOT NULL,
    sr_datetime_start  DATETIME YEAR TO MINUTE NOT NULL,
    sr_datetime_end    DATETIME YEAR TO MINUTE,
    sr_location        VARCHAR(128) NOT NULL
);

-- Theater pool (pool_no=1, ret_date=05/10/2026)
INSERT INTO session_resources VALUES (0, 1, 1, 'Torres, Rosa',    'coordinator', '2026-05-10 08:00', '2026-05-10 17:00', 'Downtown Theater - Lobby');
INSERT INTO session_resources VALUES (0, 2, 1, 'Green, Marcus',   'usher',       '2026-05-10 08:00', '2026-05-10 17:00', 'Downtown Theater - Lobby');
INSERT INTO session_resources VALUES (0, 3, 1, 'Silva, Ana',      'usher',       '2026-05-10 08:00', '2026-05-10 17:00', 'Downtown Theater - Lobby');
INSERT INTO session_resources VALUES (0, 4, 1, 'Harris, Denise',  'security',    '2026-05-10 08:00', '2026-05-10 17:00', 'Downtown Theater - Entry');

-- Sports pool (pool_no=2, ret_date=05/24/2026)
INSERT INTO session_resources VALUES (0, 1, 2, 'Chang, David',    'coordinator', '2026-05-24 09:00', '2026-05-24 18:00', 'Lakefront Stadium - Gate A');
INSERT INTO session_resources VALUES (0, 2, 2, 'O''Brien, Patrick','usher',      '2026-05-24 09:00', '2026-05-24 18:00', 'Lakefront Stadium - Gate A');
INSERT INTO session_resources VALUES (0, 3, 2, 'Young, Carl',     'security',    '2026-05-24 09:00', '2026-05-24 18:00', 'Lakefront Stadium - Gate A');
INSERT INTO session_resources VALUES (0, 4, 2, 'Harris, Denise',  'security',    '2026-05-24 09:00', '2026-05-24 18:00', 'Lakefront Stadium - Gate B');
