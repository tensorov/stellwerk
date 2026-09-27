//! # Контракт Stellwerk
//!
//! Общие типы федерации: события, задачи, рабочие места, привязка модели, evidence,
//! RBAC и модули. Это **единственное** место, где зафиксирована форма данных, по
//! которой общаются реестр, исполнитель, хранилище и gateway.
//!
//! ## Правила файла
//!
//! * Меняется только packet'ом с меткой `contract-change` (см. `AGENTS.md`).
//! * Предикаты политик (`can_transition_to`, `is_terminal`, `allows`,
//!   `permits_model`, `permits`, `required_level`) **реализованы** пакетами M1
//!   (сессия #2, 2026-09-27). Раньше они были `todo!()` — намеренной
//!   RED-спецификацией; тесты в `tests/` писались до правил и при их появлении
//!   падали, а не проходили. Именно поэтому тесты нельзя было писать по коду.
//! * Никаких `Default` там, где отсутствие значения меняет смысл: пустое
//!   ограничение сети и «сеть разрешена» — не одно и то же.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Risk
// ═══════════════════════════════════════════════════════════════════════════

/// Класс риска операции. Упорядочен: `R0 < R1 < … < R5`.
///
/// Класс назначается операции, а не модулю: один и тот же MCP-сервер может
/// содержать и read-only инструмент (R0), и инструмент с внешним эффектом (R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskClass {
    /// R0 — чтение локальных данных, без записи и без сети. Пример: поиск по
    /// уже проиндексированному тексту, чтение файла из песочницы.
    R0,
    /// R1 — запись в пределы рабочего места: правка файлов, создание артефактов
    /// в песочнице. Откатывается удалением песочницы.
    R1,
    /// R2 — сетевой выход: HTTP-запросы во внешний мир, скачивание файлов.
    /// Наружу уходят только данные, помеченные как `DataClass::Public`.
    R2,
    /// R3 — доступ к секретам: чтение credential-хранилищ, расшифровка токена,
    /// подстановка ключа провайдера. Требует явного подтверждения владельца.
    R3,
    /// R4 — внешний наблюдаемый эффект: создание PR, публикация пакета, отправка
    /// сообщения. Эффект виден другим людям и не откатывается песочницей.
    R4,
    /// R5 — необратимое разрушение: удаление данных вне песочницы, force push,
    /// миграция с потерей. Требует подтверждения и явной записи в evidence.
    R5,
}

impl RiskClass {
    /// Минимальный уровень, на котором операция этого класса допускается без
    /// дополнительных условий. Требует реализации (RED-спецификация).
    pub fn required_level(self) -> PolicyLevel {
        match self {
            RiskClass::R0 => PolicyLevel::L0Local,
            RiskClass::R1 => PolicyLevel::L2Contributor,
            RiskClass::R2 => PolicyLevel::L3Maintainer,
            RiskClass::R3 => PolicyLevel::L5Owner,
            RiskClass::R4 => PolicyLevel::L5Owner,
            RiskClass::R5 => PolicyLevel::L5Owner,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Events
// ═══════════════════════════════════════════════════════════════════════════

/// Источник события. Различает «человек сказал», «агент сказал» и «система решила».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// Человек через UI или CLI.
    Manual,
    /// Агент в opencode (планирование, ревью).
    OpenCode,
    /// Доска задач.
    Kanban,
    /// Стартовая последовательность (загрузка конфигурации, self-check).
    Startup,
    /// Календарный триггер.
    Calendar,
    /// Внешний агент-шлюз (Hermes).
    Hermes,
    /// Внутреннее событие федерации (таймер, сбор метрик, guard).
    System,
}

/// Событие федерации — единица потока. События неизменяемы и append-only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    /// Уникальный идентификатор события.
    pub id: String,
    /// Откуда пришло.
    pub source: EventSource,
    /// Тип события в свободной форме (`task.delegated`, `run.loop_detected`).
    /// Не перечисление: конкретные типы закрепляют пакеты, а не этот контракт.
    pub event_type: String,
    /// Кто совершил действие: идентификатор пользователя, агента или `system`.
    pub actor: String,
    /// Рабочее место, в контексте которого произошло событие. `None` — федеративное.
    pub workspace: Option<String>,
    /// Класс риска операции, породившей событие.
    pub risk: RiskClass,
    /// Полезная нагрузка. Плоская карта, чтобы не тащить зависимости в ядро.
    #[serde(default)]
    pub payload: BTreeMap<String, String>,
    /// Время в RFC 3339 (UTC). Ядро не приводит время к типу: хранение
    /// принадлежит слою store, контракт фиксирует только формат.
    pub ts: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Task
// ═══════════════════════════════════════════════════════════════════════════

/// Класс чувствительности данных. Маршрутизирует модель, а не документирует её.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    /// Публичные данные. Допускают любой исполнитель, включая внешние модели.
    Public,
    /// Внутренние данные проекта. Не уходят провайдеру с чужим retention.
    Internal,
    /// Персональные данные. Никогда не уходят провайдеру с чужим retention,
    /// никогда не попадают в дешёвый fallback вроде Big Pickle.
    Personal,
}

/// Статус задачи. Жизненный цикл — единственное место, где определено, что
/// считается продвижением, а что — откатом.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaskStatus {
    /// Черновик: спецификация есть, план не подтверждён.
    Draft,
    /// План принят, ожидает ревью.
    Planned,
    /// План отревьюен, ожидает делегации.
    Reviewed,
    /// Передан исполнителю.
    Delegated,
    /// Идёт прогон.
    Running,
    /// Приостановлен с сохранением состояния.
    Suspended,
    /// Доказательства приложены, ожидают проверки.
    EvidenceSubmitted,
    /// Доказательства проверены инструментом.
    Verified,
    /// Валидировано человеком или gate'ом.
    Validated,
    /// Закрыта успешно.
    Done,
    /// Провалилась.
    Failed,
    /// Заблокирована: не хватило данных, полномочий или контекста. Не путать
    /// с `Failed`: `Blocked` — это запрос решения, а не вердикт о неудаче.
    Blocked,
}

impl TaskStatus {
    /// Разрешён ли переход `self → next`. Требует реализации (RED-спецификация).
    ///
    /// Инварианты, которые обязаны выполняться: терминальные `Done`/`Failed` не
    /// имеют исходящих переходов; переход в самого себя запрещён; `EvidenceSubmitted`
    /// достижим только из `Running`/`Suspended`.
    pub fn can_transition_to(self, next: TaskStatus) -> bool {
        match (self, next) {
            // Терминальные состояния: исходящих переходов нет.
            (TaskStatus::Done, _) | (TaskStatus::Failed, _) => false,

            // Переход в себя запрещён.
            (s, next) if s == next => false,

            // Прямая цепочка: Draft → Planned → Reviewed → Delegated → Running
            (TaskStatus::Draft, TaskStatus::Planned) => true,
            (TaskStatus::Planned, TaskStatus::Reviewed) => true,
            (TaskStatus::Reviewed, TaskStatus::Delegated) => true,
            (TaskStatus::Delegated, TaskStatus::Running) => true,

            // Running ↔ Suspended
            (TaskStatus::Running, TaskStatus::Suspended) => true,
            (TaskStatus::Suspended, TaskStatus::Running) => true,

            // Delegated → Suspended (приостановка до старта прогона)
            (TaskStatus::Delegated, TaskStatus::Suspended) => true,

            // EvidenceSubmitted достижим только из Running/Suspended
            (TaskStatus::Running, TaskStatus::EvidenceSubmitted) => true,
            (TaskStatus::Suspended, TaskStatus::EvidenceSubmitted) => true,

            // EvidenceSubmitted → Verified → Validated
            (TaskStatus::EvidenceSubmitted, TaskStatus::Verified) => true,
            (TaskStatus::Verified, TaskStatus::Validated) => true,

            // Validated → Done
            (TaskStatus::Validated, TaskStatus::Done) => true,

            // Running/Suspended/Delegated → Failed/Blocked
            (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Blocked) => true,
            (TaskStatus::Suspended, TaskStatus::Failed)
            | (TaskStatus::Suspended, TaskStatus::Blocked) => true,
            (TaskStatus::Delegated, TaskStatus::Failed)
            | (TaskStatus::Delegated, TaskStatus::Blocked) => true,

            // Blocked → Draft / Blocked → Planned
            (TaskStatus::Blocked, TaskStatus::Draft)
            | (TaskStatus::Blocked, TaskStatus::Planned) => true,

            // Все остальные переходы запрещены.
            _ => false,
        }
    }

    /// Терминальный ли статус.
    pub fn is_terminal(self) -> bool {
        matches!(self, TaskStatus::Done | TaskStatus::Failed)
    }
}

/// Потолки ресурсов. Часть спецификации задачи, а не настройка рантайма:
/// воркер не может их поднять, он может только упереться в них.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskLimits {
    /// Максимум изменяемых файлов.
    pub max_files: u32,
    /// Максимум строк в diff.
    pub max_diff_lines: u32,
    /// Потолок длительности прогона.
    pub timeout_seconds: u64,
    /// Максимум повторов при неуспехе.
    pub max_retries: u32,
    /// Бюджет контекста: сюда входит всё — схемы модулей, промпт, история.
    pub context_budget_tokens: u64,
    /// Класс чувствительности данных задачи.
    pub data_class: DataClass,
}

/// Спецификация задачи (AX-inspired). Это то, что `apply` кладёт в хранилище.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSpec {
    /// Идентификатор задачи; по нему идёт evidence.
    pub id: String,
    /// Чего добиться, одной фразой.
    pub goal: String,
    /// Идентификатор рабочего места (см. `WorkspaceSpec`).
    pub workspace_ref: String,
    /// Идентификатор привязки модели (см. `ModelBinding`).
    pub model_ref: String,
    /// Текст задания для исполнителя.
    pub packet: String,
    /// Потолки ресурсов.
    pub limits: TaskLimits,
    /// Политика доказательства: чем закрывать задачу.
    pub evidence_policy: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Workspace
// ═══════════════════════════════════════════════════════════════════════════

/// Сетевая политика рабочего места. `Deny` — значение по умолчанию для воркеров.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// Сеть запрещена полностью.
    Deny,
    /// Разрешены только перечисленные хосты (точное совпадение хоста).
    AllowList(Vec<String>),
    /// Сеть разрешена без ограничений. Требует явного решения владельца.
    Allow,
}

impl NetworkPolicy {
    /// Разрешён ли URL. Требует реализации (RED-спецификация).
    ///
    /// Инвариант: `Deny` запрещает **любой** URL, включая пустую строку.
    pub fn allows(&self, url: &str) -> bool {
        match self {
            NetworkPolicy::Allow => true,
            NetworkPolicy::Deny => false,
            NetworkPolicy::AllowList(hosts) => {
                // Extract host: substring between "://" and first of "/ ? #".
                // file:// has empty host.
                let host = url
                    .split_once("://")
                    .map(|(_, rest)| match rest.find(['/', '?', '#']) {
                        Some(end) => &rest[..end],
                        None => rest,
                    })
                    .unwrap_or("");
                hosts.iter().any(|h| h == host)
            }
        }
    }
}

/// Спецификация рабочего места: что агент видит и что ему разрешено.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSpec {
    /// Идентификатор рабочего места.
    pub id: String,
    /// Опорный коммит. Одно число вместо имени ветки: результат привязан к
    /// воспроизводимому состоянию.
    pub base_commit: String,
    /// Видимые репозитории.
    #[serde(default)]
    pub repos: Vec<String>,
    /// Разрешённые инструменты (белый список).
    #[serde(default)]
    pub tools_allow: Vec<String>,
    /// Запрещённые инструменты. Имеет приоритет над `tools_allow`.
    #[serde(default)]
    pub tools_deny: Vec<String>,
    /// MCP-серверы, которые поднимаются в этом месте.
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// Сетевая политика.
    pub network_policy: NetworkPolicy,
    /// Явное окружение. Секреты сюда не попадают: ключи живут в credential-цепочке.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Model binding
// ═══════════════════════════════════════════════════════════════════════════

/// Кто обрабатывает данные модели. Определяет, можно ли отдавать персональные данные.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionClass {
    /// Данные не покидают инфраструктуру владельца.
    FirstParty,
    /// Провайдер сохраняет или обучает на данных (чужой retention).
    ThirdParty,
}

/// Привязка модели к задаче. Окно и бюджет — часть привязки: нельзя назначить
/// задаче с бюджетом 43k планера с окном 200k и получить неверную диагностику.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBinding {
    /// Идентификатор привязки.
    pub id: String,
    /// Провайдер (`neuraldeep`, `anthropic`, …).
    pub provider: String,
    /// Идентификатор модели у провайдера.
    pub model_id: String,
    /// Минимальный класс чувствительности, который binding способен обработать.
    pub data_class: DataClass,
    /// Кто обрабатывает данные.
    pub retention: RetentionClass,
    /// Окно контекста в токенах.
    pub context_window: u64,
    /// Потолок ответа в токенах.
    pub max_output: u64,
    /// Бюджет на задачу в токенах.
    pub budget_tokens_per_task: u64,
}

impl DataClass {
    /// Допустимо ли назначить binding для данных этого класса.
    /// Требует реализации (RED-спецификация).
    ///
    /// Инвариант: `Personal` не допускает `RetentionClass::ThirdParty` — это и есть
    /// запрет на «personal → Big Pickle».
    pub fn permits_model(self, binding: &ModelBinding) -> bool {
        self <= binding.data_class
            && (binding.retention == RetentionClass::FirstParty || self == DataClass::Public)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Evidence (совместимо с PolyForge)
// ═══════════════════════════════════════════════════════════════════════════

/// Вид записи в цепочке доказательств. Имена совпадают с PolyForge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EvidenceKind {
    /// Утверждение модели. Само по себе ничего не доказывает.
    ModelClaim,
    /// Результат запуска инструмента. Единственное, что делает запись доказательством.
    ToolAttestation,
    /// Результат валидации (gate, ревью, сверка).
    Validation,
}

/// Запись цепочки доказательств.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEntry {
    /// Идентификатор записи.
    pub id: String,
    /// К какой задаче относится.
    pub task_id: String,
    /// Вид записи.
    pub kind: EvidenceKind,
    /// Команда, породившая запись. Для `ModelClaim` — пустая строка.
    pub command: String,
    /// Хеш результата: удостоверение, что результат не подменён.
    pub result_hash: String,
    /// Время в RFC 3339 (UTC).
    pub ts: String,
}

/// Итог проверки этапного gate'а.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    /// Прошёл ли gate.
    pub passed: bool,
    /// Чего не хватило для прохождения.
    #[serde(default)]
    pub missing: Vec<String>,
    /// Сколько записей проверено.
    pub verified: u64,
    /// Хвост цепочки: за ним виден весь путь доказательств.
    pub chain_tail_hash: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// RBAC
// ═══════════════════════════════════════════════════════════════════════════

/// Уровень полномочий. Упорядочен: `L0Local < L1SharedRead < … < L5Owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PolicyLevel {
    /// L0Local — только своё локальное рабочее место, без публикации наружу.
    L0Local,
    /// L1SharedRead — чтение общего рабочего места.
    L1SharedRead,
    /// L2Contributor — запись в общее рабочее место, запуск задач.
    L2Contributor,
    /// L3Maintainer — сопровождение: обновление модулей, запуск пайплайнов.
    L3Maintainer,
    /// L4Admin — управление участниками и правами.
    L4Admin,
    /// L5Owner — владелец: смена политик, передача владения.
    L5Owner,
}

/// Действие, которое требует полномочий.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Читать рабочее место.
    ReadWorkspace,
    /// Писать в рабочее место.
    WriteWorkspace,
    /// Запускать задачу.
    RunTask,
    /// Обновлять модуль реестра.
    UpdateModule,
    /// Управлять участниками.
    ManageMembers,
    /// Менять политики.
    GrantRoles,
}

impl PolicyLevel {
    /// Допускает ли уровень действие. Требует реализации (RED-спецификация).
    ///
    /// Инварианты: только `L5Owner` меняет политики; `ManageMembers` требует `L4Admin`;
    /// `ReadWorkspace` доступен с `L0Local`.
    pub fn permits(self, action: Action) -> bool {
        let required = match action {
            Action::ReadWorkspace => PolicyLevel::L0Local,
            Action::WriteWorkspace => PolicyLevel::L2Contributor,
            Action::RunTask => PolicyLevel::L2Contributor,
            Action::UpdateModule => PolicyLevel::L3Maintainer,
            Action::ManageMembers => PolicyLevel::L4Admin,
            Action::GrantRoles => PolicyLevel::L5Owner,
        };
        self >= required
    }
}

/// Членство в рабочем месте.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceMembership {
    /// Идентификатор пользователя.
    pub user_id: String,
    /// Идентификатор рабочего места.
    pub workspace_id: String,
    /// Уровень полномочий в этом месте.
    pub level: PolicyLevel,
}

// ═══════════════════════════════════════════════════════════════════════════
// Module (общие с stellwerk-registry)
// ═══════════════════════════════════════════════════════════════════════════

/// Вид модуля. Имена и регистр сериализации совпадают с `modules.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    /// MCP-сервер: внешний процесс, отдающий инструменты.
    Mcp,
    /// Skill: текстовая инструкция, встроенная в контекст.
    Skill,
}

/// Источник модуля. Форма совпадает с `modules.toml`; вариант без тега
/// (`untagged`), чтобы `{ git = …, rev = … }` читался как есть.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// Git-репозиторий с закреплённой ревизией.
    Git {
        /// URL репозитория.
        git: String,
        /// Закреплённая ревизия.
        rev: String,
    },
    /// npm-пакет.
    Npm {
        /// Имя пакета.
        npm: String,
        /// Версия; `None` — плавающая.
        version: Option<String>,
    },
    /// crate из crates.io.
    Crate {
        /// Имя крейта.
        #[serde(rename = "cargo")]
        krate: String,
        /// Версия; `None` — плавающая.
        version: Option<String>,
    },
}

/// Класс риска модуля. Переиспользует `RiskClass`: шкала одна на федерацию,
/// отдельная шкала для модулей означала бы две несовместимые лестницы.
pub type ModuleRisk = RiskClass;
