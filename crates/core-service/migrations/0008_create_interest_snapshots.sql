-- Снимки интересов автора, которые Claude собирает по чатам и диалогам.
CREATE TABLE interest_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source TEXT NOT NULL DEFAULT 'chats',
    summary TEXT NOT NULL DEFAULT '',
    -- [{"name": "Rust async", "weight": 8, "note": "..."}, ...]
    topics JSONB NOT NULL DEFAULT '[]'::jsonb,
    period_from TIMESTAMPTZ,
    period_to TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_interest_snapshots_created_at ON interest_snapshots (created_at DESC);
