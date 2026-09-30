//! Риск события должен пережить JSON-цикл в обе стороны и не путать «неизвестно» с «неприменимо».
//!
//! Проверяет совместимость, а не красоту формы. Главное здесь — две вещи, которые ломаются
//! молча: legacy-строка обязана читаться как `Classified` без миграции базы, а неизвестное
//! значение обязано давать ошибку, а не подставляться в `R0`.

use serde_json::json;
use stellwerk_core::contract::{EventRisk, RiskClass};

#[test]
fn legacy_bare_string_reads_as_classified() {
    assert_eq!(
        serde_json::from_value::<EventRisk>(json!("R3")).expect("legacy R3 is readable"),
        EventRisk::Classified(RiskClass::R3)
    );
    assert_eq!(
        serde_json::from_value::<EventRisk>(json!("R2")).expect("legacy R2 is readable"),
        EventRisk::Classified(RiskClass::R2)
    );
    assert_eq!(
        serde_json::from_value::<EventRisk>(json!("R0")).expect("legacy R0 is readable"),
        EventRisk::Classified(RiskClass::R0)
    );
}

#[test]
fn the_two_unknown_states_are_distinct_tokens() {
    assert_eq!(
        serde_json::from_value::<EventRisk>(json!("unclassified")).expect("unclassified"),
        EventRisk::Unclassified
    );
    assert_eq!(
        serde_json::from_value::<EventRisk>(json!("not_applicable")).expect("not applicable"),
        EventRisk::NotApplicable
    );
    assert_ne!(
        EventRisk::Unclassified,
        EventRisk::NotApplicable,
        "забыть классифицировать и риск неприменим — разные исходы (закон 6)"
    );
}

#[test]
fn classified_serializes_as_the_same_bare_string_it_reads() {
    // Ровно это делает миграцию базы ненужной: байты в events.data не меняются.
    assert_eq!(
        serde_json::to_value(EventRisk::Classified(RiskClass::R3)).unwrap(),
        json!("R3")
    );
    assert_eq!(
        serde_json::to_value(EventRisk::Unclassified).unwrap(),
        json!("unclassified")
    );
    assert_eq!(
        serde_json::to_value(EventRisk::NotApplicable).unwrap(),
        json!("not_applicable")
    );
}

#[test]
fn unknown_value_is_an_error_and_never_becomes_r0() {
    let err = serde_json::from_value::<EventRisk>(json!("R9")).expect_err("R9 is not a class");
    assert!(
        !err.to_string().is_empty(),
        "ошибка должна называть проблему, а не быть пустой"
    );
    assert!(
        serde_json::from_value::<EventRisk>(json!(null)).is_err(),
        "отсутствие значения — ошибка, а не R0: иначе «неизвестно» неотличимо от «не риск»"
    );
    assert!(
        serde_json::from_value::<EventRisk>(json!({})).is_err(),
        "пустой объект не должен превращаться в Classified(R0)"
    );
}

#[test]
fn round_trip_preserves_every_variant() {
    for original in [
        EventRisk::Classified(RiskClass::R0),
        EventRisk::Classified(RiskClass::R5),
        EventRisk::Unclassified,
        EventRisk::NotApplicable,
    ] {
        let wire = serde_json::to_value(original).unwrap();
        let back = serde_json::from_value::<EventRisk>(wire).expect("round trip");
        assert_eq!(original, back, "round trip changed the value: {original:?}");
    }
}
