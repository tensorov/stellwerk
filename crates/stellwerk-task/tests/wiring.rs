//! M4-01: шлюз достижим из `stellwerk-task`.
//!
//! До этой зависимости `decide()` жил в `stellwerk-gateway`, а `stellwerk-task`
//! от gateway не зависел: ноль вхождений в `Cargo.toml`. Слой разрешений был
//! полностью покрыт тестами и при этом недостижим из рантайма — «безопасный
//! шлюз», стоящий рядом с работающим воркером и ни на что не влияющий (F-10).
//!
//! Тест намеренно скромный: он доказывает **импортируемость и вызываемость**
//! решения из крейта task, а не то, что раннер его вызывает. Второе — предмет
//! M4-02/03, и пока `runner.rs` не создан (его создаёт M2-09), утверждать
//! обратное было бы проверкой несуществующего кода.

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest, Verdict};
use stellwerk_gateway::permission::ToolRisk;

fn request(risk: ToolRisk, caller: PolicyLevel) -> CallRequest {
    CallRequest {
        tool: "write_note".to_string(),
        module: "notes".to_string(),
        risk,
        caller,
        data: DataClass::Public,
        model: ModelBinding {
            id: "b".to_string(),
            provider: "neuraldeep".to_string(),
            model_id: "m".to_string(),
            data_class: DataClass::Public,
            retention: RetentionClass::FirstParty,
            context_window: 200_000,
            max_output: 8_000,
            budget_tokens_per_task: 43_000,
        },
        interactive: true,
    }
}

#[test]
fn gateway_decide_is_callable_from_task_crate() {
    let verdict = decide(&request(ToolRisk::Classified(RiskClass::R2), PolicyLevel::L3Maintainer));
    assert_eq!(verdict, Verdict::Ask);
}

#[test]
fn gateway_denies_irreversible_risk_from_task_crate() {
    // R5 безусловен при любом уровне, включая L5Owner: если бы раннер мог до него
    // дотянуться, необратимое разрушение делегировалось бы агенту.
    let verdict = decide(&request(ToolRisk::Classified(RiskClass::R5), PolicyLevel::L5Owner));
    assert!(
        matches!(verdict, Verdict::Deny { .. }),
        "R5 должен быть Deny, получено {verdict:?}"
    );
}

#[test]
fn gateway_denies_unclassified_tool_from_task_crate() {
    // Отсутствие данных не превращается в разрешение: неизвестный инструмент
    // не проходит, и воркер узнаёт об этом раньше, чем что-то запустит.
    let verdict = decide(&request(ToolRisk::Unclassified, PolicyLevel::L0Local));
    assert!(
        matches!(verdict, Verdict::Deny { .. }),
        "Unclassified должен быть Deny, получено {verdict:?}"
    );
}
