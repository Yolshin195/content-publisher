CREATE TABLE publication_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES publication_tasks(id) ON DELETE CASCADE,
    attempt_no INT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failure')),
    error_message TEXT,
    gateway_response JSONB,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_publication_logs_task_id ON publication_logs (task_id);
