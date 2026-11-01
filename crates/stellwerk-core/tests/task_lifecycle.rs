//! RED-спецификация: таблица переходов статусов.
//!
//! Предикаты `can_transition_to` / `is_terminal` объявлены `todo!()` в контракте,
//! поэтому тесты компилируются и падают. Это и есть приказ для исполнителя:
//! реализовать таблицу переходов, ничего не меняя в тестах.

use stellwerk_core::contract::TaskStatus;

/// Полный список статусов: тест перечисляет их явно, чтобы добавление нового
/// статуса в контракт заставляло автора обновить этот файл.
const ALL: [TaskStatus; 16] = [
    TaskStatus::Draft,
    TaskStatus::Planned,
    TaskStatus::Reviewed,
    TaskStatus::Delegated,
    TaskStatus::Running,
    TaskStatus::Suspended,
    TaskStatus::AwaitingHumanValidation,
    TaskStatus::EvidenceSubmitted,
    TaskStatus::Verified,
    TaskStatus::Validated,
    TaskStatus::HumanValidated,
    TaskStatus::Refuted,
    TaskStatus::Cancelled,
    TaskStatus::Done,
    TaskStatus::Failed,
    TaskStatus::Blocked,
];

/// Терминальные статусы перечислены явно: `is_terminal` не имеет права быть
/// единственным источником этого списка, иначе тест проверял бы предикат им же.
const TERMINAL: [TaskStatus; 5] = [
    TaskStatus::Done,
    TaskStatus::Failed,
    TaskStatus::HumanValidated,
    TaskStatus::Refuted,
    TaskStatus::Cancelled,
];

#[test]
fn happy_path_chain_is_allowed() {
    // Путь от черновика до закрытия — единственный «нормальный» маршрут.
    let chain = [
        TaskStatus::Draft,
        TaskStatus::Planned,
        TaskStatus::Reviewed,
        TaskStatus::Delegated,
        TaskStatus::Running,
        TaskStatus::EvidenceSubmitted,
        TaskStatus::Verified,
        TaskStatus::Validated,
        TaskStatus::Done,
    ];
    for pair in chain.windows(2) {
        assert!(
            pair[0].can_transition_to(pair[1]),
            "{:?} → {:?} обязан быть разрешён",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn suspend_and_resume_are_allowed() {
    assert!(TaskStatus::Running.can_transition_to(TaskStatus::Suspended));
    assert!(TaskStatus::Suspended.can_transition_to(TaskStatus::Running));
    // Приостановка допустима и до старта прогона.
    assert!(TaskStatus::Delegated.can_transition_to(TaskStatus::Suspended));
}

#[test]
fn blocked_can_reopen_as_draft() {
    // Blocked — это запрос решения, а не тупик: его обязано можно переоткрыть.
    assert!(TaskStatus::Blocked.can_transition_to(TaskStatus::Draft));
    assert!(TaskStatus::Blocked.can_transition_to(TaskStatus::Planned));
}

#[test]
fn any_running_task_may_fail_or_block() {
    for from in [
        TaskStatus::Running,
        TaskStatus::Suspended,
        TaskStatus::Delegated,
    ] {
        assert!(
            from.can_transition_to(TaskStatus::Failed),
            "{from:?} → Failed"
        );
        assert!(
            from.can_transition_to(TaskStatus::Blocked),
            "{from:?} → Blocked"
        );
    }
}

#[test]
fn evidence_requires_a_run_first() {
    // EvidenceSubmitted достижим только из прогона: без прогона нечего
    // предъявлять. Это защищает от «доказательства из воздуха».
    assert!(!TaskStatus::Draft.can_transition_to(TaskStatus::EvidenceSubmitted));
    assert!(!TaskStatus::Planned.can_transition_to(TaskStatus::EvidenceSubmitted));
    assert!(!TaskStatus::Reviewed.can_transition_to(TaskStatus::EvidenceSubmitted));
    assert!(!TaskStatus::Delegated.can_transition_to(TaskStatus::EvidenceSubmitted));
}

#[test]
fn terminal_states_have_no_exits() {
    for terminal in TERMINAL {
        for target in ALL {
            assert!(
                !terminal.can_transition_to(target),
                "{terminal:?} → {target:?} обязан быть запрещён"
            );
        }
        assert!(
            terminal.is_terminal(),
            "{terminal:?} обязан быть терминальным"
        );
    }
}

#[test]
fn non_terminal_states_are_not_reported_terminal() {
    for status in ALL {
        if TERMINAL.contains(&status) {
            continue;
        }
        assert!(
            !status.is_terminal(),
            "{status:?} не должен быть терминальным"
        );
    }
}

#[test]
fn self_transition_is_never_allowed() {
    for status in ALL {
        assert!(
            !status.can_transition_to(status),
            "{status:?} → сам себя обязан быть запрещён"
        );
    }
}

#[test]
fn cannot_skip_the_run_step() {
    // Delegated → Verified «перепрыгивает» прогон и проверку доказательств.
    assert!(!TaskStatus::Delegated.can_transition_to(TaskStatus::Verified));
    assert!(!TaskStatus::Planned.can_transition_to(TaskStatus::Validated));
    assert!(!TaskStatus::Draft.can_transition_to(TaskStatus::Done));
}

#[test]
fn validation_does_not_reopen_to_running() {
    // После валидации прогона задача не должна молча уйти обратно в работу.
    assert!(!TaskStatus::Validated.can_transition_to(TaskStatus::Running));
    assert!(!TaskStatus::Verified.can_transition_to(TaskStatus::Running));
}

#[test]
fn status_serializes_in_pascal_case() {
    assert_eq!(
        serde_json::to_value(TaskStatus::EvidenceSubmitted).unwrap(),
        serde_json::json!("EvidenceSubmitted")
    );
    let restored: TaskStatus = serde_json::from_str("\"EvidenceSubmitted\"").expect("must parse");
    assert_eq!(restored, TaskStatus::EvidenceSubmitted);
}

#[test]
fn human_validation_and_refutation_are_explicit_states() {
    assert!(TaskStatus::Suspended.can_transition_to(TaskStatus::AwaitingHumanValidation));
    assert!(TaskStatus::AwaitingHumanValidation.can_transition_to(TaskStatus::HumanValidated));
    assert!(TaskStatus::EvidenceSubmitted.can_transition_to(TaskStatus::Refuted));

    // Отмена — явный конец из активных состояний, а не молчаливое исчезновение.
    assert!(TaskStatus::Running.can_transition_to(TaskStatus::Cancelled));
    assert!(TaskStatus::Blocked.can_transition_to(TaskStatus::Cancelled));

    // Прогон не является решением человека: обойти ожидание нельзя.
    assert!(
        !TaskStatus::Running.can_transition_to(TaskStatus::HumanValidated),
        "Running → HumanValidated обязан быть запрещён: прогон не заменяет решение человека"
    );
    // Опровержение выносится по доказательствам, а не по идущему прогону.
    assert!(
        !TaskStatus::Running.can_transition_to(TaskStatus::Refuted),
        "Running → Refuted обязан быть запрещён: опровергать нечего до EvidenceSubmitted"
    );
}

#[test]
fn evidence_bindings_are_typed_and_absent_explicitly() {
    use stellwerk_core::contract::{
        CommitSha, EvidenceEntry, EvidenceKind, PolicyDigest, RecipeDigest,
    };

    let entry = EvidenceEntry {
        id: "e-1".to_owned(),
        task_id: "t-1".to_owned(),
        kind: EvidenceKind::ModelClaim,
        command: String::new(),
        result_hash: "h".to_owned(),
        ts: "2026-01-01T00:00:00Z".to_owned(),
        recipe_digest: Some(RecipeDigest("r".to_owned())),
        policy_digest: Some(PolicyDigest("p".to_owned())),
        model_fingerprint: None,
        commit_sha: Some(CommitSha("c".to_owned())),
    };

    let json = serde_json::to_value(&entry).expect("serialize");
    assert_eq!(json["recipe_digest"], serde_json::json!("r"));
    assert_eq!(json["policy_digest"], serde_json::json!("p"));
    assert_eq!(
        json["model_fingerprint"],
        serde_json::Value::Null,
        "неприменимая привязка — явный null, а не пустая строка"
    );
    let restored: EvidenceEntry = serde_json::from_value(json).expect("parse");
    assert_eq!(restored, entry);
}
