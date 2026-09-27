//! Tests for `decide` — the В2/В4 axis: `Unclassified` against interactivity,
//! and `Suspended` against `Deny`.
//!
//! RED: `decide` is a stub, so every test below fails while the suite compiles.
//! That is the intended state of M3/packet-03b.
//!
//! Ось риска R0-R5 и ось данных живут в `tests/gate.rs` — см.
//! M3/packet-03a. Стенд здесь продублирован намеренно: общий `tests/common/mod.rs`
//! дал бы packet-03a четыре файла в `files_allowed` и ~223 строки, то есть
//! одновременно C2 и лимит diff.

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest, Verdict};
use stellwerk_gateway::permission::ToolRisk;

fn binding(data_class: DataClass, retention: RetentionClass) -> ModelBinding {
    ModelBinding {
        id: "b".into(),
        provider: "neuraldeep".into(),
        model_id: "m".into(),
        data_class,
        retention,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn request() -> CallRequest {
    CallRequest {
        tool: "write_note".into(),
        module: "notes".into(),
        risk: ToolRisk::Classified(RiskClass::R0),
        caller: PolicyLevel::L2Contributor,
        data: DataClass::Public,
        model: binding(DataClass::Public, RetentionClass::FirstParty),
        interactive: true,
    }
}

/// На этих осях решение принимают только риск и уровень.
fn risky(risk: RiskClass, caller: PolicyLevel) -> CallRequest {
    CallRequest {
        risk: ToolRisk::Classified(risk),
        caller,
        ..request()
    }
}

/// Спросить некого: всё, что требует подтверждения, уходит в `Suspended`.
fn stalled(risk: RiskClass, caller: PolicyLevel) -> CallRequest {
    CallRequest {
        risk: ToolRisk::Classified(risk),
        caller,
        interactive: false,
        ..request()
    }
}

/// Инструмент не классифицирован — отдельное состояние, а не нулевой риск.
fn unknown(caller: PolicyLevel, interactive: bool) -> CallRequest {
    CallRequest {
        risk: ToolRisk::Unclassified,
        caller,
        interactive,
        ..request()
    }
}

/// L5Owner намеренно: даже владелец не отправит `Personal` в чужой retention.
fn routed(data: DataClass, retention: RetentionClass) -> CallRequest {
    let model = binding(data, retention);
    CallRequest {
        data,
        caller: PolicyLevel::L5Owner,
        model,
        ..request()
    }
}

fn is_deny(v: &Verdict) -> bool {
    matches!(v, Verdict::Deny { .. })
}

#[test]
fn unclassified_in_noninteractive_run_is_deny() {
    // Suspend неуместен: спросить некого, а «пока подумаем» — не отказ и не
    // разрешение. Правильный ответ — классифицировать тул в modules.toml.
    let v = decide(&unknown(PolicyLevel::L5Owner, false));
    assert!(is_deny(&v), "Suspend неуместен без интерактива: {v:?}");
}

#[test]
fn unclassified_interactive_at_owner_is_ask() {
    // В2: человек видит имя инструмента и аргументы — единственный путь, где
    // неизвестный риск ведёт к Ask, а не к Deny.
    assert_eq!(decide(&unknown(PolicyLevel::L5Owner, true)), Verdict::Ask);
}

#[test]
fn unclassified_interactive_below_owner_is_deny_not_ask() {
    let v = decide(&unknown(PolicyLevel::L4Admin, true));
    assert!(is_deny(&v), "Спросить может только владелец: {v:?}");
}

#[test]
fn ask_in_noninteractive_run_is_suspended_not_deny() {
    assert_eq!(
        decide(&stalled(RiskClass::R2, PolicyLevel::L3Maintainer)),
        Verdict::Suspended
    );
}

#[test]
fn suspended_settles_into_deny_when_the_ask_window_expires() {
    // В4: таймаут (24ч по умолчанию, 4ч для R4) — дефолт по истечении окна, а не
    // решение. Итог обязан стать `Deny`; `Ask` (вопрос жив) и вечный `Suspended`
    // недопустимы. Входа часами у `decide` нет — это закрывает packet-04.
    let before_timeout = decide(&stalled(RiskClass::R4, PolicyLevel::L5Owner));
    assert_eq!(before_timeout, Verdict::Suspended);
    assert_ne!(before_timeout, Verdict::Ask);
}

#[test]
fn suspended_and_deny_are_distinguishable_states() {
    // В4: «никто не ответил» и «запрещено политикой» — разные ситуации, и в
    // статусе они должны различаться, иначе расследование их не различит.
    let suspended = decide(&stalled(RiskClass::R2, PolicyLevel::L3Maintainer));
    let denied = decide(&risky(RiskClass::R1, PolicyLevel::L0Local));
    assert_eq!(suspended, Verdict::Suspended);
    let Verdict::Deny { reason } = denied else {
        panic!("R1 при L0Local обязан быть Deny");
    };
    assert!(!reason.is_empty());
}

#[test]
fn every_deny_carries_a_non_empty_reason() {
    // `Deny` обязан нести причину: без неё агент не поймёт, что делать. Все три
    // источника отказа: риск, полномочия, данные.
    let denials = [
        decide(&risky(RiskClass::R5, PolicyLevel::L5Owner)),
        decide(&risky(RiskClass::R1, PolicyLevel::L0Local)),
        decide(&routed(DataClass::Personal, RetentionClass::ThirdParty)),
    ];
    for v in &denials {
        let Verdict::Deny { reason } = v else {
            panic!("ожидался Deny, получено {v:?}");
        };
        assert!(
            !reason.trim().is_empty(),
            "Deny без причины бесполезен агенту: {v:?}"
        );
    }
}
