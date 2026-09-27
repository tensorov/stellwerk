-- Добавить колонку payload в events, чтобы roundtrip сохранял JSON-нагрузку.
-- Колонка nullable: старые записи могут не иметь payload.

ALTER TABLE events ADD COLUMN payload TEXT;
