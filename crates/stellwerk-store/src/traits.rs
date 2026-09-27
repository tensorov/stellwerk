//! Трейты хранения. Это единственная разрешённая точка доступа к состоянию.
//!
//! Требования к трейтам, зафиксированные контрактом:
//!
//! * **async** — вызов может пересечь границу БД/сети, значит нельзя блокировать
//!   рантайм. Каждая сигнатура возвращает `impl Future + Send`: `Send` обязателен,
//!   иначе хранилище нельзя держать в задаче, которую перемещают между потоками.
//! * **Send + Sync** — трейты реализуются разделяемыми структурами: несколько
//!   воркеров пишут события параллельно (см. `tests/concurrent.rs`).
//! * **ошибки через `StoreError`** — не через `String` и не через `anyhow`:
//!   вызывающий код обязан различать «нет такой задачи» и «база недоступна».

use std::future::Future;

use serde::{Deserialize, Serialize};
use stellwerk_core::contract::{Event, ModuleKind, RiskClass, Source, TaskSpec, TaskStatus};

/// Ошибка хранилища. Различает семантические случаи, потому что они требуют
/// разных реакций: `NotFound` — это путь, `Conflict` — это ошибка планирования,
/// `Sqlite` — это инфраструктура.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Запрошенной записи нет.
    #[error("not found: {0}")]
    NotFound(String),
    /// Запись уже существует с другим содержимым.
    #[error("conflict: {0}")]
    Conflict(String),
    /// Данные не прошли сериализацию или десериализацию.
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Ошибка драйвера БД. Текст, а не источник: драйвер не clon'ится.
    #[error("sqlite: {0}")]
    Sqlite(String),
    /// Миграция не применилась.
    #[error("migration: {0}")]
    Migration(String),
    /// Операция ещё не реализована — RED-спецификация.
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

/// Запись реестра модулей в том виде, в каком её хранит федерация.
///
/// Это проекция `stellwerk_registry::model::Module`, а не его копия: крейт store
/// не зависит от registry (иначе получился бы цикл), поэтому повторяются только
/// поля, нужные для хранения и выборок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleRecord {
    /// Имя модуля — первичный ключ в реестре.
    pub name: String,
    /// Вид модуля.
    pub kind: ModuleKind,
    /// Класс риска, если назначен.
    pub risk: Option<RiskClass>,
    /// Источник модуля.
    pub source: Source,
    /// Команда запуска для MCP-модулей.
    pub command: Option<String>,
    /// Инструменты, которые модуль добавляет.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Измеренная цена модуля в токенах контекста.
    pub context_cost_tokens: u64,
}

/// Хранилище задач.
pub trait TaskStore: Send + Sync {
    /// Создаёт задачу. Возвращает сохранённую спецификацию.
    fn create(&self, spec: TaskSpec) -> impl Future<Output = Result<TaskSpec, StoreError>> + Send;

    /// Читает задачу по идентификатору.
    fn get(&self, id: &str) -> impl Future<Output = Result<Option<TaskSpec>, StoreError>> + Send;

    /// Меняет статус задачи. Отсутствие задачи — `NotFound`.
    fn update_status(
        &self,
        id: &str,
        status: TaskStatus,
    ) -> impl Future<Output = Result<TaskSpec, StoreError>> + Send;

    /// Все задачи в статусе.
    fn list_by_status(
        &self,
        status: TaskStatus,
    ) -> impl Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send;

    /// Все задачи рабочего места.
    fn list_by_workspace(
        &self,
        workspace_id: &str,
    ) -> impl Future<Output = Result<Vec<TaskSpec>, StoreError>> + Send;
}

/// Хранилище событий: append-only поток.
pub trait EventStore: Send + Sync {
    /// Добавляет событие в конец потока.
    fn append(&self, event: Event) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// События с момента `ts` включительно, по возрастанию времени.
    fn list_since(&self, ts: &str) -> impl Future<Output = Result<Vec<Event>, StoreError>> + Send;

    /// События конкретной задачи.
    fn list_by_task(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Vec<Event>, StoreError>> + Send;
}

/// Хранилище модулей реестра.
pub trait ModuleStore: Send + Sync {
    /// Читает модуль по имени.
    fn get(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<ModuleRecord>, StoreError>> + Send;

    /// Все модули, по имени.
    fn list(&self) -> impl Future<Output = Result<Vec<ModuleRecord>, StoreError>> + Send;

    /// Создаёт или заменяет запись модуля (upsert). Идемпотентно по имени.
    fn upsert_lock_entry(
        &self,
        record: ModuleRecord,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
}
