-- VenueInter application database (Postgres)
-- Tasks, tickets, status reviews, and sessions for background operations,
-- support workflow, CEO review queue, and persistent session storage.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS tasks (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_sub       TEXT        NOT NULL,
    user_email     TEXT,
    description    TEXT        NOT NULL,
    task_type      TEXT        NOT NULL,
    task_params    JSONB       NOT NULL DEFAULT '{}',
    status         TEXT        NOT NULL DEFAULT 'pending',
    result_summary TEXT,
    error_detail   TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS tickets (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id     UUID        REFERENCES tasks(id),
    user_sub    TEXT        NOT NULL,
    user_email  TEXT,
    status      TEXT        NOT NULL DEFAULT 'pending_assignment',
    description TEXT        NOT NULL,
    admin_notes TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- CEO review queue: excuse and disqualification requests.
-- Admin preps each case (pulls Informix data, verifies docs) then sends
-- to CEO. CEO only sees records with status = 'pending_ceo'.
CREATE TABLE IF NOT EXISTS status_reviews (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    part_no         TEXT        NOT NULL,
    pool_no         TEXT        NOT NULL,
    part_key        TEXT        NOT NULL UNIQUE,  -- part_no||'_'||pool_no
    review_type     TEXT        NOT NULL CHECK (review_type IN ('excuse', 'disqualify')),
    status          TEXT        NOT NULL DEFAULT 'pending_admin'
                               CHECK (status IN ('pending_admin', 'pending_ceo', 'completed', 'sent_back')),
    admin_notes     TEXT,
    ceo_notes       TEXT,
    decision        TEXT        CHECK (decision IN ('requalify', 'disqualify', 'permanent_excuse', 'temporary_excuse', NULL)),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_to_ceo_at  TIMESTAMPTZ,
    decided_at      TIMESTAMPTZ,
    decided_by      TEXT        -- user sub of CEO who decided
);

-- Full audit trail for every action on a status review.
CREATE TABLE IF NOT EXISTS review_history (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    status_review_id UUID        REFERENCES status_reviews(id),
    part_no          TEXT        NOT NULL,
    review_type      TEXT        NOT NULL,
    action           TEXT        NOT NULL,
    actor_sub        TEXT        NOT NULL,
    actor_email      TEXT,
    notes            TEXT,
    acted_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- App configuration / feature flags (e.g. CEO review live/maintenance state).
CREATE TABLE IF NOT EXISTS app_config (
    key         TEXT        PRIMARY KEY,
    value       TEXT        NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by  TEXT
);

INSERT INTO app_config (key, value) VALUES ('ceo_review_state', 'live')
    ON CONFLICT (key) DO NOTHING;

-- Informix sync queue: deferred writes back to the National system.
-- CEO decisions and admin actions write here immediately (fast PG commit),
-- then a cron job processes this queue and applies the changes to Informix
-- via ODBC. This keeps the CEO response time fast and gives the dashboard
-- a clear view of sync health without requiring email alerts.
--
-- Operations: 'update_pool_member_status', 'close_review_record', 'send_review_record'
-- Payload: JSON containing exactly the fields needed for the Informix UPDATE.
CREATE TABLE IF NOT EXISTS informix_sync_queue (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    operation    TEXT        NOT NULL,
    payload      JSONB       NOT NULL,
    status       TEXT        NOT NULL DEFAULT 'pending'
                             CHECK (status IN ('pending', 'completed', 'failed')),
    attempts     INT         NOT NULL DEFAULT 0,
    last_error   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tasks_user_sub         ON tasks(user_sub);
CREATE INDEX IF NOT EXISTS idx_tasks_status           ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tickets_user_sub       ON tickets(user_sub);
CREATE INDEX IF NOT EXISTS idx_tickets_task_id        ON tickets(task_id);
CREATE INDEX IF NOT EXISTS idx_tickets_status         ON tickets(status);
CREATE INDEX IF NOT EXISTS idx_status_reviews_part_no ON status_reviews(part_no);
CREATE INDEX IF NOT EXISTS idx_status_reviews_status  ON status_reviews(status);
CREATE INDEX IF NOT EXISTS idx_review_history_part_no ON review_history(part_no);
CREATE INDEX IF NOT EXISTS idx_review_history_rev_id  ON review_history(status_review_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_status      ON informix_sync_queue(status)
    WHERE status != 'completed';

-- Document cache: TIF files pulled from the WebDAV server and stored locally
-- so the CEO review page can serve them without hitting the national system
-- on every request. fetch_status: 'pending' | 'cached' | 'failed'.
CREATE TABLE IF NOT EXISTS document_cache (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    part_no      TEXT        NOT NULL,
    webdav_path  TEXT        NOT NULL UNIQUE,  -- file_path || file_name from part_image
    file_name    TEXT        NOT NULL,
    data         BYTEA,
    fetch_status TEXT        NOT NULL DEFAULT 'pending',
    fetch_error  TEXT,
    fetched_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_document_cache_part_no ON document_cache(part_no);

-- Note: the `tower_sessions` table (for persistent session storage) is created
-- automatically at startup by PostgresStore::migrate(). It does not need to
-- be defined here.
