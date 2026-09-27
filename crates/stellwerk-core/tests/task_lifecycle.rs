//! RED-спецификация: таблица переходов статусов.
//!
//! Предикаты `can_transition_to` / `is_terminal` объявлены `todo!()` в контракте,
//! поэтому тесты компилируются и падают. Это и есть приказ для исполнителя:
//! реализовать таблицу переходов, ничего не меняя в тестах.

use stellwerk_core::contract::TaskStatus;

/// Полный список статусов: тест перечисляет их явно, чтобы добавление нового
/// статуса в контракт заставляло автора обновить этот файл.
const ALL: [TaskStatus; 12] = [
    TaskStatus::Draft,
    TaskStatus::Planned,
    TaskStatus::Reviewed,
    TaskStatus::Delegated,
    TaskStatus::Running,
    TaskStatus::Suspended,
    TaskStatus::EvidenceSubmitted,
    TaskStatus::Verified,
    TaskStatus::Validated,
    TaskStatus::Done,
    TaskStatus::Failed,
    TaskStatus::Blocked,
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
    for terminal in [TaskStatus::Done, TaskStatus::Failed] {
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
        if status == TaskStatus::Done || status == TaskStatus::Failed {
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
