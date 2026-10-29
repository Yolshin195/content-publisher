-- Древовидный справочник промптов. Узел — либо папка, либо промпт с уникальным ключом,
-- по которому Claude получает его в запланированных задачах ("возьми rust_news_template").
CREATE TABLE prompt_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id UUID REFERENCES prompt_nodes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('folder', 'prompt')),
    key TEXT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    version INT NOT NULL DEFAULT 1,
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kind = 'folder' OR key IS NOT NULL)
);

CREATE UNIQUE INDEX idx_prompt_nodes_key ON prompt_nodes (lower(key)) WHERE key IS NOT NULL;
CREATE INDEX idx_prompt_nodes_parent ON prompt_nodes (parent_id);

-- Неизменяемая история: каждое сохранение текста промпта — новая версия.
CREATE TABLE prompt_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    prompt_id UUID NOT NULL REFERENCES prompt_nodes(id) ON DELETE CASCADE,
    version INT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    change_note TEXT,
    author TEXT NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (prompt_id, version)
);
