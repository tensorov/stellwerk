//! RED M4/packet-02: раннер ОБЯЗАН спросить разрешение на КАЖДЫЙ вызов инструмента,
//! а инструмент без классификации обязан получить `Deny`.
//!
//! Здесь проверяется не решение `decide` — оно закрыто в `stellwerk-gateway`
//! (`tests/gate.rs`, `tests/gate_ask.rs`), — а ДОСТИЖИМОСТЬ гейта из пути раннера.
//! Поэтому ожидаемое значение берётся из самого `decide` по тому же `CallRequest`,
//! который собирает пакет 03: своей копии правил в тесте быть не должно, иначе
//! тест проверял бы сам себя.
//!
//! Ассерты — ПО ВАРИАНТУ (`matches!`/`assert_eq` на конкретный вариант), а не флаг:
//! `Unclassified` обязан быть ОТЛИЧИМ от `R0` (закон 6), а «никто не ответил» —
//! от «запрещено политикой» (В4). Проверка вида «ошибка или нет» стёрла бы ровно
//! то различие, ради которого тест написан.

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::runner::{authorize, Call, CallVerdict};

/// Каждый риск, включая отсутствие классификации: матрица без `Unclassified` не
/// проверяет закон 6, а без `R5` — что политика вообще отказывает.
const RISKS: [ToolRisk; 7] = [
    ToolRisk::Unclassified,
    ToolRisk::Classified(RiskClass::R0),
    ToolRisk::Classified(RiskClass::R1),
    ToolRisk::Classified(RiskClass::R2),
    ToolRisk::Classified(RiskClass::R3),
    ToolRisk::Classified(RiskClass::R4),
    ToolRisk::Classified(RiskClass::R5),
];

const LEVELS: [PolicyLevel; 6] = [
    PolicyLevel::L0Local,
    PolicyLevel::L1SharedRead,
    PolicyLevel::L2Contributor,
    PolicyLevel::L3Maintainer,
    PolicyLevel::L4Admin,
    PolicyLevel::L5Owner,
];

/// Binding, пропускающий любой класс данных: ось данных проверяет тест гейта, и
/// здесь отказ по ней замаскировал бы отказ по риску.
fn holds_any_data() -> ModelBinding {
    ModelBinding {
        id: "b".into(),
        provider: "neuraldeep".into(),
        model_id: "m".into(),
        data_class: DataClass::Personal,
        retention: RetentionClass::FirstParty,
        context_window: 200_000,
        max_output: 8_000,
        budget_tokens_per_task: 43_000,
    }
}

fn call(risk: ToolRisk, interactive: bool) -> Call {
    Call {
        tool: "bash".into(),
        module: "fs".into(),
        risk,
        data: DataClass::Public,
        model: holds_any_data(),
        interactive,
    }
}

/// Тот же вызов глазами гейта — ровно то, что пакет 03 собирает из `Call` и `caller`.
fn as_request(call: &Call, caller: PolicyLevel) -> CallRequest {
    CallRequest {
        tool: call.tool.clone(),
        module: call.module.clone(),
        risk: call.risk,
        caller,
        data: call.data,
        model: call.model.clone(),
        interactive: call.interactive,
    }
}

/// Один вызов — одно решение: так каждый сценарий читается как «вход → вердикт».
fn authorize_call(risk: ToolRisk, interactive: bool, caller: PolicyLevel) -> CallVerdict {
    authorize(&call(risk, interactive), caller)
}

/// Причина отказа. Вариант обязан быть `Deny`: пустая строка не сказала бы агенту,
/// что делать, и отказ стал бы неотличим от молчания (F-22: вызов без следа).
fn denial_reason(verdict: &CallVerdict) -> &str {
    match verdict {
        CallVerdict::Deny { reason } => reason,
        other => panic!("не Deny, а {other:?}: причину взять не из чего"),
    }
}

#[test]
fn unclassified_tool_is_denied_even_for_the_owner() {
    let v = authorize_call(ToolRisk::Unclassified, false, PolicyLevel::L5Owner);
    assert!(
        matches!(v, CallVerdict::Deny { .. }),
        "спросить некого: {v:?}"
    );
    assert_ne!(v, CallVerdict::Allow, "неизвестный тул — не нулевой риск");
    assert_ne!(v, CallVerdict::Ask, "вопрос без интерактива не задан");
}

#[test]
fn unclassified_tool_is_denied_below_the_policy_owner_even_interactively() {
    let v = authorize_call(ToolRisk::Unclassified, true, PolicyLevel::L3Maintainer);
    let why = "классифицировать инструмент вправе владелец политики";
    assert!(matches!(v, CallVerdict::Deny { .. }), "{why}: {v:?}");
    assert_ne!(
        v,
        CallVerdict::Ask,
        "спросить не того — отказ даже не вынесен"
    );
}

#[test]
fn r5_is_denied_even_for_the_owner_and_even_interactively() {
    let v = authorize_call(
        ToolRisk::Classified(RiskClass::R5),
        true,
        PolicyLevel::L5Owner,
    );
    let why = "необратимое разрушение не делегируется агенту ни при каком уровне";
    assert!(matches!(v, CallVerdict::Deny { .. }), "{why}: {v:?}");
    assert_ne!(v, CallVerdict::Ask, "R5 не спрашивается: force-push руками");
}

#[test]
fn r2_below_the_contract_level_is_denied_before_any_question() {
    let v = authorize_call(
        ToolRisk::Classified(RiskClass::R2),
        true,
        PolicyLevel::L2Contributor,
    );
    assert!(
        matches!(v, CallVerdict::Deny { .. }),
        "уровень ниже нужного: {v:?}"
    );
    assert_ne!(v, CallVerdict::Ask, "нехватка уровня отказывает ДО Ask");
}

#[test]
fn r2_at_the_contract_level_asks_when_someone_can_answer() {
    let v = authorize_call(
        ToolRisk::Classified(RiskClass::R2),
        true,
        PolicyLevel::L3Maintainer,
    );
    assert_eq!(v, CallVerdict::Ask, "спросить есть кого, и вопрос задан");
}

#[test]
fn r0_at_a_contributor_is_allowed() {
    let v = authorize_call(
        ToolRisk::Classified(RiskClass::R0),
        true,
        PolicyLevel::L2Contributor,
    );
    assert_eq!(
        v,
        CallVerdict::Allow,
        "запрещено не всё, иначе доказано пессимизм"
    );
}

#[test]
fn unclassified_and_r0_are_two_states_and_not_one_verdict() {
    let at = PolicyLevel::L2Contributor;
    let unknown = authorize_call(ToolRisk::Unclassified, true, at);
    let read_only = authorize_call(ToolRisk::Classified(RiskClass::R0), true, at);
    assert_ne!(
        unknown, read_only,
        "при одном уровне тул и чтение сошлись — закон 6"
    );
    assert!(
        matches!(unknown, CallVerdict::Deny { .. }),
        "нет данных → отказ: {unknown:?}"
    );
    assert_eq!(read_only, CallVerdict::Allow, "известное чтение разрешено");
}

#[test]
fn no_answer_is_suspended_and_not_a_policy_refusal() {
    let v = authorize_call(
        ToolRisk::Classified(RiskClass::R2),
        false,
        PolicyLevel::L3Maintainer,
    );
    assert_eq!(
        v,
        CallVerdict::Suspended,
        "спросить некому — это не отказ (В4)"
    );
    assert_ne!(v, CallVerdict::Ask, "вопрос без интерактива не задан");
}

#[test]
fn a_refusal_names_its_reason_so_the_refusal_can_be_recorded() {
    let unknown = authorize_call(ToolRisk::Unclassified, false, PolicyLevel::L5Owner);
    let r5 = authorize_call(
        ToolRisk::Classified(RiskClass::R5),
        true,
        PolicyLevel::L5Owner,
    );
    for (what, verdict) in [("неклассифицированный", unknown), ("R5", r5)] {
        let why = "отказ без причины нечем записать, и он неотличим от молчания";
        assert!(!denial_reason(&verdict).trim().is_empty(), "{what}: {why}");
    }
}

/// Главный тест пакета: у `authorize` НЕТ пути, возвращающего вердикт в обход
/// `decide`. F-10 вырос не из «забыли вызвать», а из того, что прямой вызов был
/// возможен, но никто его не сделал; равенство гейту по всей матрице входов
/// закрывает класс, а не сегодняшний экземпляр.
#[test]
fn every_verdict_is_the_gate_verdict_so_allow_has_no_second_source() {
    let (mut allowed, mut denied) = (0usize, 0usize);
    for risk in RISKS {
        for caller in LEVELS {
            for interactive in [false, true] {
                let call = call(risk, interactive);
                let got = authorize(&call, caller);
                let want = decide(&as_request(&call, caller));
                let why = "раннер не вправе решить иначе, чем гейт";
                assert_eq!(
                    got, want,
                    "{why}: {risk:?} при {caller:?}, интерактив={interactive}"
                );
                match got {
                    CallVerdict::Allow => allowed += 1,
                    CallVerdict::Deny { .. } => denied += 1,
                    _ => {}
                }
            }
        }
    }
    assert!(
        allowed > 0,
        "матрица без Allow доказала бы только пессимизм"
    );
    assert!(denied > 0, "матрица без Deny доказала бы только оптимизм");
}
