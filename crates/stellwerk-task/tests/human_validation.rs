//! V1/o03: переход `AwaitingHumanValidation → HumanValidated` требует ЯВНОЙ
//! человеческой записи.
//!
//! Тишина, пустой `task_id`, пустой или пробельный `subject` — это отсутствие
//! решения, а не решение. Слить их с согласием человека значило бы выдать
//! неотличимый от разрешения дефект за приёмку (закон 6). Фикстура, помеченная
//! `FIXTURE_VALIDATION_LABEL`, решением человека не является по-прежнему.

use stellwerk_task::evidence::{
    evidence_status, model_claim, tool_attestation, validation, EvidenceEntry, EvidenceStatus,
    FIXTURE_VALIDATION_LABEL,
};

const TASK: &str = "t-human-validation";
/// Время фиксировано: решение человека проверяется, а не время его записи.
const TS: &str = "2026-11-02T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test human_validation";
/// Предмет человеческого решения: кто и что решил. Кладётся в `command` записи
/// `Validation` — отдельного поля «кто проверял» у `EvidenceEntry` нет.
const HUMAN_SUBJECT: &str = "owner reviewed the diff and accepted";

/// Удостоверенная цепочка без человеческой `Validation`: предъявлять есть что,
/// но решения человека ещё нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        model_claim(TASK, "e-claim", "h-claim", TS),
        tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS),
    ]
}

#[test]
fn transition_requires_explicit_record() {
    // Given: цепочка с удостоверением и без человеческой Validation.
    let chain = attested();
    assert_eq!(
        chain.len(),
        2,
        "фикстура состоит из утверждения и удостоверения: {chain:?}"
    );
    // When/Then: без записи человек не принял — результат ждёт человека.
    assert_eq!(
        evidence_status(&chain),
        EvidenceStatus::AwaitingHumanValidation,
        "удостоверение без решения человека не продвигает статус"
    );

    // When: добавлена человеческая Validation с непустыми task_id и subject.
    let mut validated = chain.clone();
    validated.push(validation(TASK, "e-human", HUMAN_SUBJECT, "h-tool", TS));
    // Then: только явная запись разрешает переход.
    assert_eq!(
        evidence_status(&validated),
        EvidenceStatus::HumanValidated,
        "человеческая Validation с task_id и subject — явное решение"
    );

    // When: та же Validation, но без task_id.
    let mut no_task = chain.clone();
    no_task.push(validation("", "e-no-task", HUMAN_SUBJECT, "h-tool", TS));
    // Then: запись без задачи не идентифицирует решение — переход запрещён.
    assert_eq!(
        evidence_status(&no_task),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation без task_id — не человеческое решение"
    );

    // When: subject пуст.
    let mut empty_subject = chain.clone();
    empty_subject.push(validation(TASK, "e-empty", "", "h-tool", TS));
    // Then: решение не названо — это не запись, а её отсутствие.
    assert_eq!(
        evidence_status(&empty_subject),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation с пустым subject ничего не решает"
    );

    // When: subject состоит только из пробелов.
    let mut whitespace_subject = chain.clone();
    whitespace_subject.push(validation(TASK, "e-space", "   ", "h-tool", TS));
    // Then: пробелы — не решение.
    assert_eq!(
        evidence_status(&whitespace_subject),
        EvidenceStatus::AwaitingHumanValidation,
        "Validation с пробельным subject ничего не решает"
    );

    // When: единственная Validation — явно помеченная фикстура.
    let mut fixture = chain;
    fixture.push(validation(
        TASK,
        "e-fixture",
        FIXTURE_VALIDATION_LABEL,
        "h-tool",
        TS,
    ));
    // Then: фикстура остаётся не-решением, даже с task_id.
    assert_eq!(
        evidence_status(&fixture),
        EvidenceStatus::AwaitingHumanValidation,
        "метка фикстуры не выдаёт тест за приёмку"
    );
}
