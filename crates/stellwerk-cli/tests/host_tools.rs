//! a02: хост применяет к workspace только `AuthorizedCall`.
//!
//! Гейт выдаёт разрешение записи отдельным состоянием: `GateDecision::Authorized`
//! несёт сам `AuthorizedCall`, а `Allow`/`Deny`/`Ask`/`Suspended`/`Unclassified`
//! такого значения не несут. Поэтому голое разрешение нельзя применить как запись —
//! не по соглашению, а потому что применять нечего.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use stellwerk_cli::build_runner::{apply_gate_decision, HostRefusal};
use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{CallRequest, ContentDigest};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{
    decide_request, AllowBasis, GateAnswer, GatePolicy, GateRequest, PolicyVerdict,
};
use stellwerk_task::m5::session_identity::{
    Adjudication, CodemodeState, SessionDescription, ToolSet,
};

fn binding() -> ModelBinding {
    ModelBinding {
        id: "b1".to_owned(),
        provider: "neuraldeep".to_owned(),
        model_id: "qwen3.6-unlim-xl-noreason".to_owned(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 138_336,
        max_output: 41_500,
        budget_tokens_per_task: 100_000,
    }
}

/// Таблица рисков разрешает R1: пишущий вызов с полным аргументом и digest.
struct WriteRisk;

impl GatePolicy for WriteRisk {
    fn request_for(&self, call: &Adjudication) -> Option<CallRequest> {
        Some(CallRequest {
            tool: call.tool.clone(),
            module: "host-tools".to_owned(),
            risk: ToolRisk::Classified(RiskClass::R1),
            caller: PolicyLevel::L2Contributor,
            data: DataClass::Public,
            model: binding(),
            interactive: false,
        })
    }

    fn decide_call(&self, call: &Adjudication, _path: Option<&str>) -> PolicyVerdict {
        match self.request_for(call) {
            Some(request) => PolicyVerdict::Evaluated {
                request,
                basis: AllowBasis::RiskTable,
            },
            None => PolicyVerdict::Unclassified,
        }
    }
}

fn session(held: &[&str]) -> SessionDescription {
    SessionDescription::new(
        "host-tools",
        &std::env::temp_dir(),
        ToolSet::listed(held).expect("набор инструментов"),
        Duration::from_secs(60),
        CodemodeState::AbsentAfterConfigLoad,
    )
    .expect("описание сессии")
}

fn request(tool: &str, path: &str, argument: &str, claimed: &ContentDigest) -> GateRequest {
    GateRequest {
        kind: "tool_call".to_owned(),
        tool: tool.to_owned(),
        parent_tool_call_id: None,
        path: Some(path.to_owned()),
        argument: Some(argument.to_owned()),
        content_digest: Some(claimed.as_hex().to_owned()),
    }
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(name: &str) -> PathBuf {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("sw-a02-{name}-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("рабочее место");
    root
}

type Decision = stellwerk_task::m5::gate_channel::GateDecision;

fn decide(tool: &str, path: &str, argument: &str, claimed: &ContentDigest) -> Decision {
    decide_request(
        &session(&["read", "write", "edit"]),
        &WriteRisk,
        &request(tool, path, argument, claimed),
    )
}

#[test]
fn write_with_mismatched_digest_is_refused() {
    let root = scratch("mismatch");
    std::fs::write(root.join("src/note.txt"), b"original").expect("файл");
    // Заявлен digest одних байт, а приходят другие: гейт обязан отказать, а хост —
    // не писать. «Путь разрешён» не значит «содержимое то же».
    let claimed = ContentDigest::of(b"authorized bytes");
    let decision = decide("write", "src/note.txt", "attacker bytes", &claimed);

    assert!(
        matches!(decision.answer(), GateAnswer::Deny { .. }),
        "несовпавший digest — отказ, а не режим: {:?}",
        decision.answer()
    );
    let refused = apply_gate_decision(&root, &decision).expect_err("хост не должен писать");
    assert!(
        matches!(refused, HostRefusal::NotAuthorized { .. }),
        "названный отказ: {refused:?}"
    );
    assert_eq!(
        std::fs::read(root.join("src/note.txt")).expect("файл на месте"),
        b"original",
        "файл не изменён"
    );
}

#[test]
fn authorized_write_leaves_the_authorized_digest() {
    let root = scratch("authorized");
    std::fs::write(root.join("src/note.txt"), b"before").expect("файл");
    let body = "after: authorized bytes";
    let claimed = ContentDigest::of(body.as_bytes());
    let decision = decide("write", "src/note.txt", body, &claimed);

    assert!(
        matches!(decision.answer(), GateAnswer::Authorized { .. }),
        "запись с digest авторизуется: {:?}",
        decision.answer()
    );
    let written = apply_gate_decision(&root, &decision).expect("авторизованная запись");
    assert_eq!(written.digest, claimed.as_hex(), "digest записи");

    let file = std::fs::read(root.join("src/note.txt")).expect("файл после записи");
    assert_eq!(
        ContentDigest::of(&file).as_hex(),
        claimed.as_hex(),
        "digest файла равен авторизованному"
    );
}

#[test]
fn a_path_outside_the_workspace_is_refused_without_writing() {
    let root = scratch("escape");
    let name = root
        .file_name()
        .expect("имя каталога")
        .to_string_lossy()
        .into_owned();
    let outside = format!("../{name}-outside.txt");
    let body = "escape";
    let claimed = ContentDigest::of(body.as_bytes());
    let decision = decide("write", &outside, body, &claimed);

    let refused = apply_gate_decision(&root, &decision).expect_err("выход за workspace");
    assert!(
        matches!(refused, HostRefusal::OutsideWorkspace { .. }),
        "названный отказ: {refused:?}"
    );
    assert!(
        !root
            .parent()
            .expect("родитель")
            .join(format!("{name}-outside.txt"))
            .exists(),
        "файл вне workspace не создан"
    );
}
