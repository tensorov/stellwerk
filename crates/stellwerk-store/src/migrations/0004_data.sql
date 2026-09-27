-- Сохраняем полную сериализованную модель события, чтобы roundtrip
-- возвращал все поля (source, event_type, actor, workspace, risk, payload, ts).
-- Денормализованные колонки (task_id, ts) остаются для индексов.

ALTER TABLE events ADD COLUMN data TEXT;
