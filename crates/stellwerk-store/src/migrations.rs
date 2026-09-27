//! Миграции схемы. Хранятся как SQL-файлы и встраиваются в бинарник через
//! `include_str!`, поэтому применённая миграция едет вместе с программой:
//! офлайн-установка не зависит от наличия каталога миграций.

/// Порядок важен: применяются строго по имени файла, без gaps.
pub const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    ("0002_indexes", include_str!("migrations/0002_indexes.sql")),
    ("0003_payload", include_str!("migrations/0003_payload.sql")),
    ("0004_data", include_str!("migrations/0004_data.sql")),
];
