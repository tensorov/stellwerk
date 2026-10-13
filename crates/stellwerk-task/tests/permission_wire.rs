//! RED M4/packet-02: раннер ОБЯЗАН спросить разрешение на КАЖДЫЙ вызов инструмента,
//! а эффективный риск берётся из ДОВЕРЕННОГО реестра, а не из самодекларации `call.risk`.
//!
//! Здесь проверяется не решение `decide` — оно закрыто в `stellwerk-gateway`
//! (`tests/gate.rs`, `tests/gate_ask.rs`), — а ДОСТИЖИМОСТЬ гейта из пути раннера.
//! Классы R0..R5 приходят из тестового реестра `TestRegistry`, а ожидаемый вердикт —
//! из самого `decide` по тому `CallRequest`, который `authorize` обязан собрать из
//! применённого реестром риска. Своей копии правил в тесте быть не должно, иначе
//! тест проверял бы сам себя.
//!
//! Ассерты — ПО ВАРИАНТУ (`matches!`/`assert_eq` на конкретный вариант), а не флаг:
//! `Unclassified` обязан быть ОТЛИЧИМ от `R0` (закон 6), а «никто не ответил» —
//! от «запрещено политикой» (В4).

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{decide, CallRequest};
use stellwerk_gateway::permission::{PermissionError, ToolRisk};
use stellwerk_task::runner::{authorize, Call, CallVerdict, RiskRegistry};

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

/// Доверенный тестовый реестр: класс риска инструмента задаёт ТОЛЬКО он. Инструмент
/// вне реестра — `Err`, а не `R0`-умолчание: неизвестное обязано быть отказом.
struct TestRegistry {
    entries: Vec<(String, String, ToolRisk)>,
}

impl TestRegistry {
    fn new() -> Self {
        Self {
            entries: vec![
                ("r0".into(), "m".into(), ToolRisk::Classified(RiskClass::R0)),
                ("r1".into(), "m".into(), ToolRisk::Classified(RiskClass::R1)),
                ("r2".into(), "m".into(), ToolRisk::Classified(RiskClass::R2)),
                ("r3".into(), "m".into(), ToolRisk::Classified(RiskClass::R3)),
                ("r4".into(), "m".into(), ToolRisk::Classified(RiskClass::R4)),
                ("r5".into(), "m".into(), ToolRisk::Classified(RiskClass::R5)),
                ("unclass".into(), "m".into(), ToolRisk::Unclassified),
            ],
        }
    }
}

impl RiskRegistry for TestRegistry {
    fn risk_of(&self, tool: &str, module: &str) -> Result<ToolRisk, PermissionError> {
        self.entries
            .iter()
            .find(|(t, m, _)| t == tool && m == module)
            .map(|(_, _, risk)| *risk)
            .ok_or(PermissionError::TableIncomplete {
                risk: RiskClass::R0,
            })
    }
}

fn test_registry() -> TestRegistry {
    TestRegistry::new()
}

/// Инструмент, чей класс в тестовом реестре равен переданному риску.
fn tool_for(risk: ToolRisk) -> (&'static str, &'static str) {
    match risk {
        ToolRisk::Unclassified => ("unclass", "m"),
        ToolRisk::Classified(RiskClass::R0) => ("r0", "m"),
        ToolRisk::Classified(RiskClass::R1) => ("r1", "m"),
        ToolRisk::Classified(RiskClass::R2) => ("r2", "m"),
        ToolRisk::Classified(RiskClass::R3) => ("r3", "m"),
        ToolRisk::Classified(RiskClass::R4) => ("r4", "m"),
        ToolRisk::Classified(RiskClass::R5) => ("r5", "m"),
    }
}

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

/// Вызов с инструментом из реестра, но с ЗАВЕДОМО нерелевантной самодекларацией:
/// решение обязано следовать реестру, а не `risk`.
fn call_for(risk: ToolRisk, interactive: bool) -> Call {
    let (tool, module) = tool_for(risk);
    Call {
        tool: tool.into(),
        module: module.into(),
        risk: ToolRisk::Unclassified,
        data: DataClass::Public,
        model: holds_any_data(),
        interactive,
    }
}

/// Тот же вызов глазами гейта — запрос собирается из применённого реестром риска,
/// а не из `call.risk`. Это ровно то, что обязан делать `authorize`.
fn as_request_applied(call: &Call, caller: PolicyLevel, reg: &dyn RiskRegistry) -> CallRequest {
    let applied = reg
        .risk_of(&call.tool, &call.module)
        .unwrap_or(ToolRisk::Unclassified);
    CallRequest {
        tool: call.tool.clone(),
        module: call.module.clone(),
        risk: applied,
        caller,
        data: call.data,
        model: call.model.clone(),
        interactive: call.interactive,
    }
}

/// Один вызов — одно решение: так каждый сценарий читается как «вход → вердикт».
fn authorize_call(risk: ToolRisk, interactive: bool, caller: PolicyLevel) -> CallVerdict {
    authorize(&call_for(risk, interactive), caller, &test_registry()).verdict
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
fn a_tool_absent_from_the_registry_is_refused_even_when_the_caller_declares_r0() {
    let reg = test_registry();
    let mut c = call_for(ToolRisk::Classified(RiskClass::R0), false);
    // Инструмент вне реестра, а вызывающий объявляет R0 — так делал CLI-путь N1.
    c.tool = "bash".into();
    c.module = "fs".into();
    c.risk = ToolRisk::Classified(RiskClass::R0);
    let decision = authorize(&c, PolicyLevel::L5Owner, &reg);
    assert!(
        matches!(decision.verdict, CallVerdict::Deny { .. }),
        "реестр не знает `bash`/`fs`, а объявленный R0 не спасает: {:?}",
        decision.verdict
    );
    assert_ne!(
        decision.verdict,
        CallVerdict::Allow,
        "самодекларация R0 не разрешает незарегистрированный инструмент"
    );
}

#[test]
fn changing_only_the_declared_risk_does_not_change_the_decision() {
    let reg = test_registry();
    let mut declared_r0 = call_for(ToolRisk::Classified(RiskClass::R4), true);
    declared_r0.risk = ToolRisk::Classified(RiskClass::R0);
    let mut declared_r5 = call_for(ToolRisk::Classified(RiskClass::R4), true);
    declared_r5.risk = ToolRisk::Classified(RiskClass::R5);

    let a = authorize(&declared_r0, PolicyLevel::L5Owner, &reg).verdict;
    let b = authorize(&declared_r5, PolicyLevel::L5Owner, &reg).verdict;
    assert_eq!(a, b, "смена самодекларации не меняет решение");
    assert_eq!(
        a,
        CallVerdict::Ask,
        "решено по реестру R4, а не по объявленным R0/R5"
    );
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
/// закрывает класс, а не сегодняшний экземпляр. Классы берутся из реестра, поэтому
/// матрица не держится на самодекларации.
#[test]
fn every_verdict_is_the_gate_verdict_so_allow_has_no_second_source() {
    let reg = test_registry();
    let (mut allowed, mut denied) = (0usize, 0usize);
    for risk in RISKS {
        for caller in LEVELS {
            for interactive in [false, true] {
                let call = call_for(risk, interactive);
                let got = authorize(&call, caller, &reg).verdict;
                let want = decide(&as_request_applied(&call, caller, &reg));
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
