//! Tests for `decide` — the risk axis (R0-R5) and the data axis.
//!
//! RED: `decide` is a stub, so every test below fails while the suite compiles.
//! That is the intended state of M3/packet-03a.
//!
//! Порядок правил важен: риск, затем полномочия, затем данные. `Deny` по любому
//! из них; иначе `Ask` при R2/R3/R4; иначе `Allow`. R5 — безусловный `Deny` (В1).
//!
//! Ось В2/В4 (`Unclassified`, интерактивность, `Suspended`) живёт в
//! `tests/gate_ask.rs` — см. M3/packet-03b.

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{
    authorize_write, decide, CallDecision, CallRequest, ContentDigest, RequestRefusal,
    RequestedCall, Verdict,
};
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
fn r0_at_l2_with_public_data_and_first_party_is_allow() {
    assert_eq!(decide(&request()), Verdict::Allow);
}

#[test]
fn r1_at_l0_is_deny_for_lacking_write_rights() {
    let v = decide(&risky(RiskClass::R1, PolicyLevel::L0Local));
    assert!(
        is_deny(&v),
        "R1 требует L2Contributor, у L0Local нет прав на запись: {v:?}"
    );
}

#[test]
fn r2_at_l2_is_deny_not_ask() {
    // R2 требует L3Maintainer, поэтому правило полномочий срабатывает раньше Ask:
    // иначе «спросили бы у L2 и разрешили», а L2 не решает сетевой выход.
    let v = decide(&risky(RiskClass::R2, PolicyLevel::L2Contributor));
    assert!(is_deny(&v), "R2 требует L3Maintainer: {v:?}");
}

#[test]
fn r2_at_l3_is_ask() {
    assert_eq!(
        decide(&risky(RiskClass::R2, PolicyLevel::L3Maintainer)),
        Verdict::Ask
    );
}

#[test]
fn r3_at_l4_is_deny_after_adr_0002() {
    // После ADR-0002 R3 требует L5Owner: L4Admin недостаточен.
    let v = decide(&risky(RiskClass::R3, PolicyLevel::L4Admin));
    assert!(is_deny(&v), "R3 требует L5Owner: {v:?}");
}

#[test]
fn r3_at_l5_is_ask_human_decides_secret_access() {
    assert_eq!(
        decide(&risky(RiskClass::R3, PolicyLevel::L5Owner)),
        Verdict::Ask
    );
}

#[test]
fn r4_at_l4_is_deny() {
    let v = decide(&risky(RiskClass::R4, PolicyLevel::L4Admin));
    assert!(is_deny(&v), "R4 требует L5Owner: {v:?}");
}

#[test]
fn r4_at_l5_is_ask_on_one_action_not_deny() {
    // В1: R4 — не безусловный запрет, а вопрос на ОДНО действие.
    assert_eq!(
        decide(&risky(RiskClass::R4, PolicyLevel::L5Owner)),
        Verdict::Ask
    );
}

#[test]
fn r5_is_deny_at_l5_owner_too() {
    // В1: R5 безусловен даже у владельца — необратимое разрушение не делегируется
    // агенту ни при каком уровне, поэтому R5 не попадает в ветку Ask.
    let v = decide(&risky(RiskClass::R5, PolicyLevel::L5Owner));
    assert!(is_deny(&v), "R5 — безусловный Deny: {v:?}");
}

#[test]
fn personal_data_to_third_party_is_deny_even_at_r0_l5() {
    let v = decide(&routed(DataClass::Personal, RetentionClass::ThirdParty));
    assert!(is_deny(&v), "Personal не уходит в чужой retention: {v:?}");
}

#[test]
fn public_data_to_third_party_is_not_denied_by_data() {
    let v = decide(&routed(DataClass::Public, RetentionClass::ThirdParty));
    assert!(!is_deny(&v), "Public допускает внешнего исполнителя: {v:?}");
}

/// Собирает пишущий вызов из фактических байтов содержимого: digest считается
/// здесь же по `content`, а не берётся из текста модели.
fn write_call(path: &str, content: &[u8]) -> RequestedCall {
    let digest = ContentDigest::of(content);
    RequestedCall::new(
        "write",
        Some(path.to_owned()),
        content.to_vec(),
        &digest,
    )
    .expect("digest посчитан по тем же байтам")
}

#[test]
fn same_tool_path_different_content_gets_different_decision() {
    // Дано: один инструмент и один путь, но разное содержимое.
    let first = write_call("notes/a.md", b"alpha");
    let second = write_call("notes/a.md", b"beta");
    assert_eq!(first.tool(), second.tool());
    assert_eq!(first.path(), second.path());
    assert_ne!(
        first.content_digest(),
        second.content_digest(),
        "разное содержимое — разные digest"
    );

    // Когда: гейт авторизует оба вызова по одному и тому же основанию.
    let authorized_first = authorize_write(&first, "plan:1");
    let authorized_second = authorize_write(&second, "plan:1");

    // Тогда: решения не совпадают. Если бы решение принималось по `(tool, path)`,
    // они были бы равны, и подмена содержимого прошла бы под тем же разрешением.
    assert_ne!(
        authorized_first, authorized_second,
        "одна пара (tool, path) с разным содержимым — разные решения"
    );
    let (CallDecision::Authorized(first), CallDecision::Authorized(second)) =
        (&authorized_first, &authorized_second)
    else {
        panic!("оба вызова с верным digest обязаны быть Authorized: {authorized_first:?} / {authorized_second:?}");
    };
    assert_ne!(
        first.scope(),
        second.scope(),
        "scope связан с digest, иначе он не различает содержимое"
    );
    assert!(first.scope().contains(first.content_digest().as_hex()));
    assert!(second.scope().contains(second.content_digest().as_hex()));
}

#[test]
fn content_replaced_after_authorization_is_refused() {
    // Дано: авторизован вызов с содержимым `alpha`.
    let digest = ContentDigest::of(b"alpha");
    let authorized = authorize_write(&write_call("notes/a.md", b"alpha"), "plan:1");
    let CallDecision::Authorized(authorized) = authorized else {
        panic!("верный digest авторизуется");
    };

    // Когда: после авторизации содержимое заменили на `beta`.
    let replaced = authorized.verify_content(b"beta");

    // Тогда: сверка digest отказывает и называет оба дайджеста — подмена не проходит.
    let error = replaced.expect_err("подмена после авторизации обязана быть отказом");
    assert!(error.to_string().contains(digest.as_hex()));
}

#[test]
fn a_claim_that_disagrees_with_the_bytes_is_refused() {
    // Дано: заявлен digest от `alpha`, а байты — `beta` (так выглядит подмена
    // содержимого до решения гейта).
    let claimed = ContentDigest::of(b"alpha");

    // Когда: из этих байтов собирают запрос.
    let built = RequestedCall::new(
        "write",
        Some("notes/a.md".to_owned()),
        b"beta".to_vec(),
        &claimed,
    );

    // Тогда: запрос не собирается — digest считается по байтам, а не принимается на слово.
    assert!(matches!(built, Err(RequestRefusal::DigestMismatch { .. })));
}

#[test]
fn content_digest_is_sha256_of_the_bytes() {
    // sha256("abc") из coreutils — независимый от реализации ориентир.
    assert_eq!(
        ContentDigest::of(b"abc").as_hex(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
