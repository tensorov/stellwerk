//! Audit: сверка манифеста с живым сервером — RED-шов (packet-08), реализация в 09.
//!
//! Шов транспортно-независим и остаётся sync (`&mut self`): M0 не тянет async-клиент,
//! а тесты не должны поднимать живой MCP-процесс. Таймаут в шов не зашит — бюджет
//! уходит в транспорт при его сборке, а в тестах моделируется исходом `Timeout`.

use std::path::Path;

use crate::model::Module;

/// Прод-бюджет на один тул; тесты инжектят 50 мс (packet-08, Контекст).
pub const AUDIT_TIMEOUT_MS: u64 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// Заглушка RED-пакета: реальный stdio/rmcp транспорт приходит в packet-09.
    #[error("audit not implemented (packet-09)")]
    NotImplemented,
    /// У модуля нет `command` — это не то же самое, что «сервер не поднялся».
    #[error("module {0} has no command")]
    NoCommand(String),
    #[error("failed to spawn MCP server")]
    Spawn,
}

/// Транспортно-независимый шов аудита.
pub trait McpProbe {
    /// `tools/list` — имена ровно те, что отдаёт сервер.
    fn list_tools(&mut self) -> Result<Vec<String>, AuditError>;
    /// `tools/call` с `{}` как invalid-args пробой (Q3): валидный JSON-RPC ответ сам по
    /// себе доказывает вызываемость — валидацию аргументов делает сервер.
    fn call_tool(&mut self, name: &str, args: &str) -> Result<CallOutcome, AuditError>;
}

/// Исход одного `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallOutcome {
    Result(serde_json::Value),
    /// JSON-RPC error — тул жив (Q3), в not_invocable не попадает.
    Error(serde_json::Value),
    Timeout,
    Malformed,
}

/// Отчёт по одному модулю; пустые векторы = «всё совпало» (exit 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReport {
    /// Заявлено в манифесте, нет на сервере.
    pub missing_in_server: Vec<String>,
    /// Есть на сервере, манифестом не заявлено.
    pub extra_in_server: Vec<String>,
    /// Заявлен, присутствует, но не вызывается (таймаут / битый ответ).
    pub not_invocable: Vec<String>,
    /// `kind = skill`: аудит отложен, spawn не выполнялся.
    pub skill_skipped: bool,
}

/// Сверка exact-set имён + вызываемости каждого заявленного тулла.
///
/// RED: `Err(NotImplemented)` намеренно — тесты ниже обязаны падать явно, а не
/// «проходить» на пустом отчёте. Зелёным это делает packet-09.
pub fn audit_manifest(
    _tools: &[String],
    _probe: &mut dyn McpProbe,
    _timeout_ms: u64,
) -> Result<AuditReport, AuditError> {
    Err(AuditError::NotImplemented)
}

/// Аудит одного модуля: `kind = skill` → пропуск, `kind = mcp` → spawn + сверка.
///
/// RED: транспорт — packet-09. `root`/`timeout_ms` уже в сигнатуре, чтобы 09 не менял
/// публичный API шва.
pub fn audit_module(
    _module: &Module,
    _root: &Path,
    _timeout_ms: u64,
) -> Result<AuditReport, AuditError> {
    Err(AuditError::NotImplemented)
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModulesFile;
    use serde_json::json;

    /// Тестовый бюджет: 50 мс вместо прод-10-секунд, чтобы таймаут был наблюдаем.
    const TEST_TIMEOUT_MS: u64 = 50;

    /// Зонд-фейк: сервер отдаёт заданный список имён, на `tools/call` — заданный исход
    /// (по умолчанию успешный result). Часов в шове нет: таймаут — это исход, а не ожидание.
    struct FakeProbe {
        server_tools: Vec<String>,
        outcomes: Vec<(&'static str, CallOutcome)>,
    }

    impl FakeProbe {
        fn new(server_tools: &[&str]) -> Self {
            Self {
                server_tools: server_tools.iter().map(|s| (*s).to_owned()).collect(),
                outcomes: Vec::new(),
            }
        }

        fn outcome(mut self, name: &'static str, outcome: CallOutcome) -> Self {
            self.outcomes.push((name, outcome));
            self
        }
    }

    impl McpProbe for FakeProbe {
        fn list_tools(&mut self) -> Result<Vec<String>, AuditError> {
            Ok(self.server_tools.clone())
        }

        fn call_tool(&mut self, name: &str, _args: &str) -> Result<CallOutcome, AuditError> {
            match self.outcomes.iter().find(|(n, _)| *n == name) {
                Some((_, outcome)) => Ok(outcome.clone()),
                None => Ok(CallOutcome::Result(json!({}))),
            }
        }
    }

    fn audit(manifest: &[&str], mut probe: FakeProbe) -> Result<AuditReport, AuditError> {
        let tools: Vec<String> = manifest.iter().map(|s| (*s).to_owned()).collect();
        audit_manifest(&tools, &mut probe, TEST_TIMEOUT_MS)
    }

    #[test]
    fn audit_exact_match_reports_no_failures() {
        let r = audit(&["a", "b", "c"], FakeProbe::new(&["a", "b", "c"])).expect("must report");
        assert!(r.missing_in_server.is_empty() && r.extra_in_server.is_empty());
        assert!(r.not_invocable.is_empty());
        assert!(!r.skill_skipped, "mcp-аудит не пропускается");
    }

    #[test]
    fn audit_missing_in_server_names_manifest_only_tool() {
        let r = audit(&["a", "b", "c"], FakeProbe::new(&["a", "b"])).expect("must report");
        assert_eq!(r.missing_in_server, vec!["c".to_owned()]);
    }

    #[test]
    fn audit_extra_in_server_names_server_only_tool() {
        let r =
            audit(&["a", "b", "c"], FakeProbe::new(&["a", "b", "c", "d"])).expect("must report");
        assert_eq!(r.extra_in_server, vec!["d".to_owned()]);
    }

    #[test]
    fn audit_not_invocable_on_timeout() {
        let p = FakeProbe::new(&["a", "b", "c"]).outcome("c", CallOutcome::Timeout);
        let r = audit(&["a", "b", "c"], p).expect("must report");
        assert_eq!(r.not_invocable, vec!["c".to_owned()]);
    }

    #[test]
    fn audit_not_invocable_on_malformed_answer() {
        let p = FakeProbe::new(&["a", "b", "c"]).outcome("b", CallOutcome::Malformed);
        let r = audit(&["a", "b", "c"], p).expect("must report");
        assert_eq!(r.not_invocable, vec!["b".to_owned()]);
    }

    #[test]
    fn audit_jsonrpc_error_is_still_invocable() {
        // Q3: invalid-args проба отбивается ошибкой, а не молчанием — тул жив.
        let p = FakeProbe::new(&["a"]).outcome("a", CallOutcome::Error(json!({"code": -32602})));
        let r = audit(&["a"], p).expect("must report");
        assert!(r.not_invocable.is_empty(), "JSON-RPC error = вызываем");
    }

    const SKILL_TOML: &str = r#"
[registry.calibration]
tokens_per_byte = 0.5
measured_at = "2026-01-01T00:00:00Z"

[module.notes]
kind = "skill"
source = { npm = "@scope/notes", version = "1.0.0" }
"#;

    #[test]
    fn audit_skill_module_is_skipped_without_spawn() {
        let mf = ModulesFile::from_toml_str(SKILL_TOML).expect("fixture must parse");
        let r = audit_module(&mf.module["notes"], Path::new("."), TEST_TIMEOUT_MS)
            .expect("skill is deferred, not an error");
        assert!(r.skill_skipped, "skill audit deferred");
        assert!(r.missing_in_server.is_empty() && r.not_invocable.is_empty());
    }

    #[test]
    fn audit_timeout_budget_is_injected_not_hardcoded() {
        // 50 мс в тесте и 10_000 мс в проде — разные числа: зашитый в шов дефолт
        // тихо утопил бы тестовый бюджет, поэтому оба значения зафиксированы.
        assert_eq!(AUDIT_TIMEOUT_MS, 10_000, "прод-дефолт зафиксирован пакетом");
        let r = audit(&["a"], FakeProbe::new(&["a"])).expect("must report under injected budget");
        assert!(r.not_invocable.is_empty());
    }

    #[test]
    fn audit_empty_manifest_and_empty_server_pass_vacuously() {
        let r = audit(&[], FakeProbe::new(&[])).expect("nothing declared, nothing to fail");
        assert!(r.missing_in_server.is_empty() && r.extra_in_server.is_empty());
        assert!(r.not_invocable.is_empty() && !r.skill_skipped);
    }
}
