//! V1/o04: `Refuted` принимается только с названным основанием и ссылкой на
//! опровергающие данные.
//!
//! Опровержение без основания неотличимо от отказа без причины: пустая строка и
//! пробелы — это отсутствие основания, а не основание (закон 6). Ссылка на
//! опровергающие данные не теряется: без неё опровержение нечем сверить с
//! данными. `Refuted` — человеческое решение о предъявленных доказательствах, а
//! не вердикт о провале прогона, поэтому он не становится `Failed`.

use stellwerk_core::contract::TaskStatus;
use stellwerk_task::evidence::{
    evidence_status, model_claim, refutation, tool_attestation, validation, EvidenceEntry,
    EvidenceStatus, RefutationError,
};

const TASK: &str = "t-refutation";
/// Время фиксировано: проверяется основание опровержения, а не время записи.
const TS: &str = "2026-11-02T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-task --test refutation";
const BASIS: &str = "the attestation hash does not match the recorded artifact";
const REFUTING_DATA: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Удостоверенная цепочка: предъявлять есть что, решения человека ещё нет.
fn attested() -> Vec<EvidenceEntry> {
    vec![
        model_claim(TASK, "e-claim", "h-claim", TS),
        tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS),
    ]
}

#[test]
fn refuted_requires_stated_basis() {
    // Given/When: основание пусто. Then: опровержение отвергнуто, а не принято
    // молча — иначе оно неотличимо от отказа без причины.
    assert_eq!(
        refutation("", REFUTING_DATA),
        Err(RefutationError::MissingBasis),
        "пустое основание — это отсутствие основания, а не основание"
    );

    // When: основание состоит только из пробелов. Then: это тоже отсутствие.
    assert_eq!(
        refutation("   ", REFUTING_DATA),
        Err(RefutationError::MissingBasis),
        "пробельное основание ничего не называет"
    );

    // When: ссылка на опровергающие данные пуста или пробельна. Then: проверять
    // опровержение не по чему — отказ.
    assert_eq!(
        refutation(BASIS, ""),
        Err(RefutationError::MissingRefutingData),
        "ссылка на опровергающие данные обязательна"
    );
    assert_eq!(
        refutation(BASIS, "  "),
        Err(RefutationError::MissingRefutingData),
        "пробельная ссылка — это отсутствие ссылки"
    );

    // Given/When: названы и основание, и ссылка. Then: опровержение принято.
    let accepted = refutation(BASIS, REFUTING_DATA).expect("основание и ссылка названы");

    // Then: опровержение — `Refuted`, а не `Failed`: провал прогона и
    // человеческое опровержение доказательств — разные факты.
    assert_eq!(
        accepted.status(),
        TaskStatus::Refuted,
        "принятое опровержение переводит задачу в Refuted"
    );
    assert_ne!(
        accepted.status(),
        TaskStatus::Failed,
        "опровержение не превращается в Failed"
    );
    assert_eq!(
        accepted.basis(),
        BASIS,
        "основание сохраняется, а не выбрасывается после проверки"
    );
    assert_eq!(
        accepted.refuting_data(),
        REFUTING_DATA,
        "ссылка на опровергающие данные не теряется"
    );

    // Given: та же цепочка плюс обычная человеческая Validation.
    let mut validated = attested();
    validated.push(validation(
        TASK,
        "e-human",
        "owner reviewed the diff and accepted",
        "h-tool",
        TS,
    ));
    // Then: обычная Validation — это принятие, а не опровержение.
    assert_eq!(
        evidence_status(&validated),
        EvidenceStatus::HumanValidated,
        "человеческая Validation остаётся принятием"
    );
    assert_ne!(
        TaskStatus::HumanValidated,
        accepted.status(),
        "принятие и опровержение — разные решения"
    );
}
