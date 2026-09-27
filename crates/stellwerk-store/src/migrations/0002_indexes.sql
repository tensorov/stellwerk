-- Индексы под запросы, которые контракт уже требует.
-- list_by_status и list_by_workspace без индекса — полное сканирование таблицы,
-- что на ноутбуке с федерацией заметно уже, чем кажется.

CREATE INDEX IF NOT EXISTS idx_tasks_status       ON tasks (status);
CREATE INDEX IF NOT EXISTS idx_tasks_workspace    ON tasks (workspace_id);
CREATE INDEX IF NOT EXISTS idx_events_task        ON events (task_id);
CREATE INDEX IF NOT EXISTS idx_events_ts          ON events (ts);
CREATE INDEX IF NOT EXISTS idx_evidence_task      ON evidence (task_id);
CREATE INDEX IF NOT EXISTS idx_modules_name       ON modules (name);
