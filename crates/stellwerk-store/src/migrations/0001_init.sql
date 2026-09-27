-- Начальная схема федерации.
-- JSON-модель: `data` хранит сериализованный тип из stellwerk-core::contract.
-- SQLite не имеет JSONB, поэтому тип JSON, а не BLOB: так данные читаются
-- внешними инструментами (sqlite3, Litestream-бэкап) без декодера.

CREATE TABLE IF NOT EXISTS workspaces (
    id          TEXT PRIMARY KEY,
    data        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id           TEXT PRIMARY KEY,
    status       TEXT NOT NULL,
    workspace_id TEXT,
    data         TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS events (
    id         TEXT PRIMARY KEY,
    task_id    TEXT,
    ts         TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS modules (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    risk       TEXT,
    data       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS evidence (
    id         TEXT PRIMARY KEY,
    task_id    TEXT NOT NULL,
    kind       TEXT NOT NULL,
    data       TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Метки применённых миграций: без них повторный запуск применил бы 0001 снова.
CREATE TABLE IF NOT EXISTS schema_migrations (
    version    TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);
