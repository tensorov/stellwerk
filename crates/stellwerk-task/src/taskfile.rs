use serde::{Deserialize, Serialize};
use stellwerk_core::contract::{DataClass, TaskSpec};
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
    /// Загрузить TaskFile из TOML-строки.
    ///
    /// **TODO (packet-02):** реализовать десериализацию.
    /// Сейчас — заглушка, всегда возвращающая ошибку.
    pub fn from_toml_str(_s: &str) -> Result<Self, TaskFileError> {
        Err(TaskFileError::Validation(
            "not yet implemented — stub".into(),
        ))
    }

    /// Конвертировать в `TaskSpec` контракта.
    ///
    /// **TODO (packet-02):** реализовать маппинг.
    pub fn into_spec(self) -> Result<TaskSpec, TaskFileError> {
        Err(TaskFileError::Validation(
            "not yet implemented — stub".into(),
        ))
    }
}
