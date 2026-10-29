-- Предложения постов от Claude и метаданные генерации.
-- suggested — предложено (ждёт решения автора), rejected — отклонено с причиной.
ALTER TABLE articles DROP CONSTRAINT IF EXISTS articles_state_check;
ALTER TABLE articles ADD CONSTRAINT articles_state_check
    CHECK (state IN ('suggested', 'draft', 'scheduled', 'archived', 'rejected'));

ALTER TABLE articles
    ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual' CHECK (origin IN ('manual', 'ai')),
    ADD COLUMN tags TEXT[] NOT NULL DEFAULT '{}',
    -- Ключ основного промпта и его версия на момент генерации.
    ADD COLUMN prompt_key TEXT,
    ADD COLUMN prompt_version INT,
    -- Все промпты, участвовавшие в генерации (включая подключённые через {{> key}}):
    -- [{"key": "...", "version": N}, ...]
    ADD COLUMN prompt_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Почему Claude предлагает этот пост.
    ADD COLUMN rationale TEXT,
    -- Источники: [{"title": "...", "url": "..."}, ...]
    ADD COLUMN sources JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Обратная связь автора (например, причина отклонения) — Claude учитывает её дальше.
    ADD COLUMN feedback TEXT,
    ADD COLUMN reviewed_at TIMESTAMPTZ,
    -- Прямая ссылка на картинку-обложку (альтернатива Media Service).
    ADD COLUMN cover_image_url TEXT;

CREATE INDEX idx_articles_state ON articles (state) WHERE deleted_at IS NULL;
CREATE INDEX idx_articles_prompt_key ON articles (lower(prompt_key)) WHERE deleted_at IS NULL;
CREATE INDEX idx_articles_tags ON articles USING GIN (tags);
