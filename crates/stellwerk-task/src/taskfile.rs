use crate::execution::{
    EngineProfile, ExecutionRefusal, ExecutionSpec, ResolvedExecution, REGISTRY,
};
use serde::{Deserialize, Serialize};
use stellwerk_core::contract::{DataClass, TaskLimits, TaskSpec};
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Error)]
pub enum TaskFileError {
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("validation error: {0}")]
    Validation(String),
    /// Поле `profile` не названо вовсе. Отдельный вариант, а не `Validation`:
    /// отсутствие данных — состояние, отличное от неверного значения, и
    /// подставленное `pi` сделало бы отказ неотличимым от согласия.
    #[error("task.toml does not name `profile`: the engine profile must be explicit")]
    MissingProfile,
    /// Профиль назван, но реестр этой сборки его не разрешает. Значение
    /// прочитано и отвергнуто — это не то же, что отсутствие поля.
    #[error("profile `{profile}` is not allowed: registry holds {known:?}")]
    UnknownProfile {
        /// Названное, но не разрешённое значение.
        profile: String,
        /// Что вообще разрешено в этой сборке.
        known: Vec<String>,
    },
    /// Программа запуска не названа: `program` пуст. Отдельное состояние, а не
    /// `MissingProfile` и не значение по умолчанию: профиль назван, но запускать
    /// нечего, и пустая строка не должна читаться как согласие на что-то.
    #[error("task does not name `program`: it is blank, so there is nothing to launch")]
    EmptyProgram,
    /// Файл не прочитан. Отдельный вариант, а не Validation: отсутствие файла —
    /// это операционная ошибка, и путать её с опечаткой в содержимом нельзя.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// ═══════════════════════════════════════════════════════════════════════════
// TOML-схема задачи
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFile {
    #[serde(rename = "task")]
    pub spec: TaskFileSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFileSpec {
    pub id: String,
    pub goal: String,
    pub workspace_ref: String,
    pub model_ref: String,
    pub packet: String,
    /// Названный движок. `None` — поля нет; это НЕ `pi` по умолчанию, а
    /// отсутствие данных, которое `resolve` превращает в `MissingProfile`.
    /// `#[serde(default)]` здесь намеренно нет: умолчание стёрло бы разницу
    /// между «не названо» и «названо явно».
    pub profile: Option<String>,
    #[serde(default)]
    pub limits: TaskFileLimits,
    #[serde(default)]
    pub evidence_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFileLimits {
    pub max_files: u32,
    pub max_diff_lines: u32,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub context_budget_tokens: u64,
    pub data_class: DataClass,
}

impl Default for TaskFileLimits {
    fn default() -> Self {
        Self {
            max_files: 0,
            max_diff_lines: 0,
            timeout_seconds: 0,
            max_retries: 0,
            context_budget_tokens: 0,
            data_class: DataClass::Internal,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API (stub — packet-02)
// ═══════════════════════════════════════════════════════════════════════════

impl TaskFile {
    /// Загрузить TaskFile из TOML-строки и проверить то, что не выражается типами.
    ///
    /// Проверки валидации существенны: `deny_unknown_fields` отвергает опечатку
    /// в имени ключа, но не отвергает `max_files = 0`, а нулевой лимит молча
    /// отключил бы ограничение задачи.
    pub fn from_toml_str(s: &str) -> Result<Self, TaskFileError> {
        let file: Self = toml::from_str(s)?;
        file.validate()?;
        Ok(file)
    }

    /// Прочитать TaskFile с диска.
    pub fn load(path: &std::path::Path) -> Result<Self, TaskFileError> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_toml_str(&raw)
    }

    /// Найти названный профиль в реестре этой сборки. Приватно: снаружи допуск
    /// идёт только через `resolve(program)`, который отдаёт найденный профиль в
    /// `execution::resolve`; второй публичный путь разрешения расходился бы с ним.
    ///
    /// Отсутствие поля (`None`) — `MissingProfile`, а не подстановка `pi`.
    /// Названное значение ищется в `REGISTRY`, единственном источнике истины о
    /// разрешённых движках; `UnknownProfile` называет отклонённое имя и то, что
    /// вообще разрешено. Двух разных отказов достаточно, чтобы «не названо» и
    /// «названо неверно» нельзя было перепутать.
    fn resolve_profile(&self) -> Result<&'static EngineProfile, TaskFileError> {
        let Some(profile) = self.spec.profile.as_deref() else {
            return Err(TaskFileError::MissingProfile);
        };
        REGISTRY
            .find(profile)
            .ok_or_else(|| TaskFileError::UnknownProfile {
                profile: profile.to_owned(),
                known: REGISTRY.engines(),
            })
    }

    /// Допустить запуск: названный профиль и программа проходят через
    /// `execution::resolve` — единственную точку допуска. Программа берётся из
    /// аргумента, движок — из разрешённого профиля.
    ///
    /// Пустая программа — `EmptyProgram`: профиль назван, но запускать нечего.
    pub fn resolve(&self, program: &str) -> Result<ResolvedExecution, TaskFileError> {
        let profile = self.resolve_profile()?;
        let spec = ExecutionSpec {
            program: program.to_owned(),
            engine: profile.engine().to_owned(),
        };
        crate::execution::resolve(Some(&spec), &REGISTRY).map_err(|refusal| match refusal {
            ExecutionRefusal::EmptyProgram => TaskFileError::EmptyProgram,
            ExecutionRefusal::UnknownProfile { engine, known } => TaskFileError::UnknownProfile {
                profile: engine,
                known,
            },
            // Недостижимо по построению: `Some(&spec)` передан всегда, поэтому
            // `Absent` вернуть нечем. Ветка оставлена явной, чтобы новая причина
            // отказа не растворилась в wildcard'е; отдельного варианта
            // `AbsentExecutionSpec` не заводим — «спецификации нет» здесь не наступает.
            ExecutionRefusal::Absent => TaskFileError::Validation(
                "execution spec absent: unreachable because resolve always passes Some(&spec)"
                    .to_owned(),
            ),
        })
    }

    fn validate(&self) -> Result<(), TaskFileError> {
        let t = &self.spec;
        // Профиль называется первым: без движка задача не исполнима, и отказ
        // обязан быть про отсутствие/неверный профиль, а не про лимиты.
        self.resolve_profile()?;
        let bad = |m: String| Err(TaskFileError::Validation(m));
        if t.id.trim().is_empty() {
            return bad("id обязан быть непустым".into());
        }
        if t.goal.trim().is_empty() {
            return bad("goal обязан быть непустым".into());
        }
        if t.limits.max_files == 0 {
            return bad("max_files обязан быть > 0".into());
        }
        if t.limits.max_diff_lines == 0 {
            return bad("max_diff_lines обязан быть > 0".into());
        }
        if t.limits.timeout_seconds == 0 {
            return bad("timeout_seconds обязан быть > 0".into());
        }
        Ok(())
    }

    /// Конвертировать в `TaskSpec` контракта.
    ///
    /// Структура `TaskSpec` здесь НЕ дублируется: она уже живёт в
    /// `stellwerk_core::contract`. Дублирование разошлось бы с контрактом при
    /// первом же изменении лимитов.
    pub fn into_spec(self) -> Result<TaskSpec, TaskFileError> {
        let t = self.spec;
        let limits = TaskLimits {
            max_files: t.limits.max_files,
            max_diff_lines: t.limits.max_diff_lines,
            timeout_seconds: t.limits.timeout_seconds,
            max_retries: t.limits.max_retries,
            context_budget_tokens: t.limits.context_budget_tokens,
            data_class: t.limits.data_class,
        };
        Ok(TaskSpec {
            id: t.id,
            goal: t.goal,
            workspace_ref: t.workspace_ref,
            model_ref: t.model_ref,
            packet: t.packet,
            limits,
            evidence_policy: t.evidence_policy,
        })
    }
}
