//! V1/v03b: bindings evidence видны проверке цепочки.
//!
//! Привязка, которую нельзя сломать подменой, ничего не привязывает. Поэтому
//! `recipe_digest`, `policy_digest` и `commit_sha` входят в хвост цепочки:
//! замена любого из них обязана дать другой хвост. Отсутствие привязки у
//! `ToolAttestation` — названный отказ, а не молчаливое «привязка не нужна»
//! (закон 6). `None` остаётся явным «неприменимо» для остальных видов записей.

use stellwerk_core::contract::{CommitSha, PolicyDigest, RecipeDigest};
use stellwerk_task::evidence::{
    evaluate, model_claim, tool_attestation, validation, EvidenceEntry,
};
use stellwerk_task::runner::{RunOutcome, StopReason, Verdict};

const TASK: &str = "v03b-vertical";
/// Время фиксировано: хвост не зависит от времени, иначе расхождение нельзя
/// отличить от подмены.
const TS: &str = "2026-11-03T00:00:00Z";
const TEST_CMD: &str = "cargo test -p stellwerk-cli --test vertical_v3";
/// Bindings ненулевые и различимые: подмена обязана отличаться от исходного
/// значения, а не совпасть с ним случайно.
const RECIPE: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const POLICY: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COMMIT: &str = "cccccccccccccccccccccccccccccccccccccccc";

fn outcome() -> RunOutcome {
    RunOutcome {
        stop_reason: StopReason::Stop,
        tool_steps: 2,
        turns: 3,
        total_input_tokens: 1_000,
        verdict: Verdict::Valid,
    }
}

/// Цепочка `ModelClaim → ToolAttestation → Validation`, где удостоверение
/// привязано ко всем трём полям. Bindings остальных звеньев остаются `None`:
/// требование пакета относится к аттестации, а не к каждому виду записи.
fn chain() -> Vec<EvidenceEntry> {
    let mut tool = tool_attestation(TASK, "e-tool", TEST_CMD, "h-tool", TS);
    tool.recipe_digest = Some(RecipeDigest(RECIPE.to_owned()));
    tool.policy_digest = Some(PolicyDigest(POLICY.to_owned()));
    tool.commit_sha = Some(CommitSha(COMMIT.to_owned()));
    vec![
        model_claim(TASK, "e-claim", "h-claim", TS),
        tool,
        validation(TASK, "e-validation", TEST_CMD, "h-tool", TS),
    ]
}

fn replace_binding(chain: &mut [EvidenceEntry], field: &str, value: Option<&str>) {
    let tool = chain
        .iter_mut()
        .find(|entry| entry.id == "e-tool")
        .expect("фикстура содержит удостоверение");
    match field {
        "recipe_digest" => tool.recipe_digest = value.map(|v| RecipeDigest(v.to_owned())),
        "policy_digest" => tool.policy_digest = value.map(|v| PolicyDigest(v.to_owned())),
        "commit_sha" => tool.commit_sha = value.map(|v| CommitSha(v.to_owned())),
        other => panic!("неизвестное binding-поле: {other}"),
    }
}

#[test]
fn tampered_digest_breaks_binding() {
    // Given: закрытая цепочка с ненулевыми bindings на аттестации.
    let base = evaluate(&outcome(), &chain());
    assert!(
        base.passed,
        "полная цепочка с bindings обязана закрывать гейт: {:?}",
        base.missing
    );

    // When/Then: подмена каждого поля по очереди обязана дать другой хвост.
    for field in ["recipe_digest", "policy_digest", "commit_sha"] {
        let mut tampered = chain();
        replace_binding(&mut tampered, field, Some("tampered"));
        let gate = evaluate(&outcome(), &tampered);
        assert_ne!(
            base.chain_tail_hash, gate.chain_tail_hash,
            "подмена {field} обязана менять хвост: иначе привязка не видна проверке"
        );
    }

    // Когда привязки нет вовсе — это названный отказ, а не «привязка не нужна».
    for field in ["recipe_digest", "policy_digest", "commit_sha"] {
        let mut missing = chain();
        replace_binding(&mut missing, field, None);
        let gate = evaluate(&outcome(), &missing);
        assert!(
            !gate.passed,
            "ToolAttestation без {field} не закрывает гейт: {:?}",
            gate.missing
        );
        assert!(
            gate.missing
                .iter()
                .any(|reason| reason.contains("ToolAttestation") && reason.contains(field)),
            "отказ обязан назвать отсутствующее поле {field}: {:?}",
            gate.missing
        );
    }
}
