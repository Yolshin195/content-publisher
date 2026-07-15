CREATE TABLE articles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    content_html TEXT NOT NULL DEFAULT '',
    excerpt TEXT,
    cover_media_id UUID,
    state TEXT NOT NULL DEFAULT 'draft' CHECK (state IN ('draft', 'scheduled', 'archived')),
    scheduled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_articles_scheduled_at ON articles (scheduled_at) WHERE deleted_at IS NULL;
