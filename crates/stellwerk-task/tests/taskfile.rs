use stellwerk_core::contract::DataClass;
use stellwerk_task::taskfile::{TaskFile, TaskFileError};

// ═══════════════════════════════════════════════════════════════════════════
// FIXTURE: валидный TOML с полным набором полей
// ═══════════════════════════════════════════════════════════════════════════

const FIXTURE_VALID: &str = r#"[task]
id = "task-001"
goal = "Add unit tests for TaskLimits"
workspace_ref = "ws-main"
model_ref = "binding-01"
packet = "packet-01"
profile = "pi"
evidence_policy = "ToolAttestation"

[task.limits]
max_files = 10
max_diff_lines = 200
timeout_seconds = 300
max_retries = 3
context_budget_tokens = 8192
data_class = "internal"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// FIXTURE: неизвестный ключ → должен быть Err (deny_unknown_fields)
// ═══════════════════════════════════════════════════════════════════════════

const FIXTURE_UNKNOWN_KEY: &str = r#"[task]
id = "task-002"
goal = "unknown key test"
workspace_ref = "ws-main"
model_ref = "binding-01"
packet = "packet-02"
bogus_field = "should fail"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// FIXTURE: отрицательное max_files (u32 не парсит минус)
// ═══════════════════════════════════════════════════════════════════════════

const FIXTURE_NEGATIVE_LIMITS: &str = r#"[task]
id = "task-003"
goal = "negative limits"
workspace_ref = "ws-main"
model_ref = "binding-01"
packet = "packet-03"

[task.limits]
max_files = -1
max_diff_lines = 200
timeout_seconds = 300
max_retries = 3
context_budget_tokens = 8192
data_class = "internal"

task.evidence_policy = "ToolAttestation"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// FIXTURE: профиль назван, но не разрешён — значение прочитано и отвергнуто
// ═══════════════════════════════════════════════════════════════════════════

const FIXTURE_WRONG_PROFILE: &str = r#"[task]
id = "task-004"
goal = "wrong profile test"
workspace_ref = "ws-main"
model_ref = "binding-01"
packet = "packet-04"
profile = "opencode"
evidence_policy = "ToolAttestation"

[task.limits]
max_files = 10
max_diff_lines = 200
timeout_seconds = 300
max_retries = 3
context_budget_tokens = 8192
data_class = "internal"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// FIXTURE: профиль не назван вовсе — отсутствие данных, а не значение
// ═══════════════════════════════════════════════════════════════════════════

const FIXTURE_MISSING_PROFILE: &str = r#"[task]
id = "task-005"
goal = "missing profile test"
workspace_ref = "ws-main"
model_ref = "binding-01"
packet = "packet-05"
evidence_policy = "ToolAttestation"

[task.limits]
max_files = 10
max_diff_lines = 200
timeout_seconds = 300
max_retries = 3
context_budget_tokens = 8192
data_class = "internal"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_taskfile_from_toml_parses_valid_fixture() {
    let result = TaskFile::from_toml_str(FIXTURE_VALID);
    // RED: stub always returns Err — test must fail here.
    let task_file = result.expect("from_toml_str should succeed for valid TOML");

    assert_eq!(task_file.spec.id, "task-001");
    assert_eq!(task_file.spec.goal, "Add unit tests for TaskLimits");
    assert_eq!(task_file.spec.workspace_ref, "ws-main");
    assert_eq!(task_file.spec.model_ref, "binding-01");
    assert_eq!(task_file.spec.packet, "packet-01");
    assert_eq!(task_file.spec.limits.max_files, 10);
    assert_eq!(task_file.spec.limits.max_diff_lines, 200);
    assert_eq!(task_file.spec.limits.timeout_seconds, 300);
    assert_eq!(task_file.spec.limits.max_retries, 3);
    assert_eq!(task_file.spec.limits.context_budget_tokens, 8192);
    assert_eq!(task_file.spec.limits.data_class, DataClass::Internal);
    assert_eq!(task_file.spec.evidence_policy, "ToolAttestation");
}

#[test]
fn test_taskfile_from_toml_rejects_unknown_key() {
    let result = TaskFile::from_toml_str(FIXTURE_UNKNOWN_KEY);
    // RED: stub always returns Err — test must fail here.
    assert!(result.is_err(), "unknown key should cause an error");
}

#[test]
fn test_taskfile_from_toml_rejects_negative_limits() {
    let result = TaskFile::from_toml_str(FIXTURE_NEGATIVE_LIMITS);
    // RED: stub always returns Err — test must fail here.
    assert!(result.is_err(), "negative max_files should cause an error");
}

/// `profile` обязан быть назван явно и может быть только `pi`. Три исхода
/// различимы по типу: `Ok` для `pi`, `UnknownProfile` для названного чужого
/// значения, `MissingProfile` для отсутствия поля. Отсутствие не читается как
/// `pi`: `MissingProfile` — отдельный вариант, а не согласие по умолчанию.
#[test]
fn profile_must_be_explicit_pi() {
    // Given/When: профиль назван и разрешён.
    let accepted =
        TaskFile::from_toml_str(FIXTURE_VALID).expect("profile = \"pi\" обязан приниматься");
    // Then: значение сохранено как названо, без подстановки.
    assert_eq!(accepted.spec.profile.as_deref(), Some("pi"));

    // When: профиль назван, но движок не разрешён.
    let wrong = TaskFile::from_toml_str(FIXTURE_WRONG_PROFILE)
        .expect_err("profile = \"opencode\" обязан быть отвергнут");
    // Then: названный отказ про неверное значение, с перечнем разрешённых.
    let TaskFileError::UnknownProfile { profile, known } = wrong else {
        panic!("ожидался UnknownProfile, получено {wrong:?}");
    };
    assert_eq!(profile, "opencode");
    assert_eq!(known, vec!["pi".to_owned()]);

    // When: поля `profile` нет вовсе.
    let missing = TaskFile::from_toml_str(FIXTURE_MISSING_PROFILE)
        .expect_err("отсутствие profile обязано быть отвергнуто");
    // Then: другой названный отказ — отсутствие, а не неверное значение.
    assert!(
        matches!(missing, TaskFileError::MissingProfile),
        "ожидался MissingProfile, получено {missing:?}"
    );
}

#[test]
fn test_taskfile_into_spec_converts_id_and_goal() {
    let result = TaskFile::from_toml_str(FIXTURE_VALID);
    // RED: stub always returns Err — test must fail here.
    let task_file = result.expect("from_toml_str should succeed for valid TOML");

    let spec = task_file.into_spec().expect("into_spec should succeed");

    assert_eq!(spec.id, "task-001");
    assert_eq!(spec.goal, "Add unit tests for TaskLimits");
}

// ═══════════════════════════════════════════════════════════════════════════
// resolve(program): допуск запуска идёт через execution::resolve
// ═══════════════════════════════════════════════════════════════════════════

/// `resolve(program)` — единственный вход допуска: названный профиль уходит в
/// `execution::resolve`, а программа возвращается как была названа вызывающим.
#[test]
fn resolve_returns_the_named_program_and_the_profile_engine() {
    let task_file = TaskFile::from_toml_str(FIXTURE_VALID).expect("валидный TOML");
    let resolved = task_file
        .resolve("/opt/pi/bin/pi")
        .expect("profile = \"pi\" обязан разрешиться");
    assert_eq!(resolved.program(), "/opt/pi/bin/pi");
    assert_eq!(resolved.engine(), "pi");
}

/// Пустая и пробельная программа — `EmptyProgram`: профиль назван, но запускать
/// нечего. Это отдельное состояние, не `MissingProfile` и не подстановка.
#[test]
fn resolve_refuses_a_blank_program_as_empty_program() {
    let task_file = TaskFile::from_toml_str(FIXTURE_VALID).expect("валидный TOML");
    for blank in ["", "   "] {
        let refusal = task_file
            .resolve(blank)
            .expect_err("пустая программа обязана быть отвергнута");
        assert!(
            matches!(refusal, TaskFileError::EmptyProgram),
            "программа {blank:?} — EmptyProgram, получено {refusal:?}"
        );
    }
}

/// Отсутствие профиля и неверное имя остаются различимыми и на входе
/// `resolve(program)`: `MissingProfile` не читается как `UnknownProfile`.
#[test]
fn resolve_keeps_missing_and_unknown_profile_distinct() {
    // `from_toml_str` здесь не годится: он валидирует и падает на самом профиле.
    // Поэтому TOML разбирается напрямую, чтобы проверить именно `resolve`.
    let missing: TaskFile = toml::from_str(FIXTURE_MISSING_PROFILE).expect("TOML разбирается");
    let missing_refusal = missing
        .resolve("/opt/pi/bin/pi")
        .expect_err("отсутствие profile обязано быть отвергнуто");
    assert!(
        matches!(missing_refusal, TaskFileError::MissingProfile),
        "ожидался MissingProfile, получено {missing_refusal:?}"
    );

    let unknown: TaskFile = toml::from_str(FIXTURE_WRONG_PROFILE).expect("TOML разбирается");
    let unknown_refusal = unknown
        .resolve("/opt/pi/bin/pi")
        .expect_err("profile = \"opencode\" обязан быть отвергнут");
    let TaskFileError::UnknownProfile { profile, known } = unknown_refusal else {
        panic!("ожидался UnknownProfile, получено {unknown_refusal:?}");
    };
    assert_eq!(profile, "opencode");
    assert_eq!(known, vec!["pi".to_owned()]);
}
