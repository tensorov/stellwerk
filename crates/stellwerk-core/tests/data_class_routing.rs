//! RED-спецификация: маршрутизация данных к модели.
//!
//! Главный инвариант проекта: персональные данные не уходят провайдеру с чужим
//! retention — то есть никогда не попадают в дешёвый внешний fallback.
//! `DataClass::permits_model` объявлен `todo!()`; тесты обязаны падать.

use stellwerk_core::contract::{DataClass, ModelBinding, RetentionClass};

fn binding(id: &str, data_class: DataClass, retention: RetentionClass) -> ModelBinding {
    ModelBinding {
        id: id.to_owned(),
        provider: "neuraldeep".to_owned(),
        model_id: id.to_owned(),
        data_class,
        retention,
        context_window: 46_112,
        max_output: 8_192,
        budget_tokens_per_task: 40_000,
    }
}

/// Персональные данные нельзя отдавать модели с чужим retention.
/// Big Pickle в `.omo/omo.jsonc` — именно такой fallback: дешёвый и внешний.
#[test]
fn personal_data_never_goes_to_third_party_retention() {
    for model in ["big-pickle", "external-cheap", "public-fallback"] {
        let target = binding(model, DataClass::Personal, RetentionClass::ThirdParty);
        assert!(
            !DataClass::Personal.permits_model(&target),
            "Personal не должен уходить в {model}"
        );
    }
}

/// Персональные данные допустимо обрабатывать своей моделью.
#[test]
fn personal_data_is_allowed_on_first_party_models() {
    let own = binding("qwen-local", DataClass::Personal, RetentionClass::FirstParty);
    assert!(DataClass::Personal.permits_model(&own));
}

/// Внутренние данные проекта — тоже не для чужих.
#[test]
fn internal_data_does_not_go_to_third_party_retention() {
    let external = binding("mimo", DataClass::Internal, RetentionClass::ThirdParty);
    assert!(!DataClass::Internal.permits_model(&external));
    let own = binding("qwen-local", DataClass::Internal, RetentionClass::FirstParty);
    assert!(DataClass::Internal.permits_model(&own));
}

/// Публичные данные допустимы где угодно — иначе нельзя использовать сильные
/// внешние модели вовсе.
#[test]
fn public_data_is_allowed_anywhere() {
    for retention in [RetentionClass::FirstParty, RetentionClass::ThirdParty] {
        let target = binding("space-bunny", DataClass::Public, retention);
        assert!(DataClass::Public.permits_model(&target));
    }
}

/// Привязка объявляет потолок чувствительности: binding, заявленный как Public,
/// не может обработать Internal даже при FirstParty retention.
#[test]
fn binding_cannot_exceed_its_declared_data_class() {
    let narrow = binding("public-only", DataClass::Public, RetentionClass::FirstParty);
    assert!(!DataClass::Internal.permits_model(&narrow));
    assert!(!DataClass::Personal.permits_model(&narrow));
    assert!(DataClass::Public.permits_model(&narrow));
}

/// Полная матрица «данные × провайдер» — чтобы правило читалось как таблица.
#[test]
fn routing_matrix() {
    let mut matrix = Vec::new();
    for data in [DataClass::Public, DataClass::Internal, DataClass::Personal] {
        for retention in [RetentionClass::FirstParty, RetentionClass::ThirdParty] {
            for declared in [DataClass::Public, DataClass::Internal, DataClass::Personal] {
                let target = binding("m", declared, retention);
                matrix.push((data, retention, declared, data.permits_model(&target)));
            }
        }
    }
    // Два правила, покрывающие всю матрицу:
    //  (1) data не выше потолка привязки;
    //  (2) Public — единственный класс, допустимый при ThirdParty.
    for (data, retention, declared, actual) in matrix {
        let expected = data <= declared
            && (retention == RetentionClass::FirstParty || data == DataClass::Public);
        assert_eq!(
            actual, expected,
            "data={data:?} retention={retention:?} declared={declared:?}"
        );
    }
}

#[test]
fn data_class_is_ordered() {
    assert!(DataClass::Public < DataClass::Internal);
    assert!(DataClass::Internal < DataClass::Personal);
}
