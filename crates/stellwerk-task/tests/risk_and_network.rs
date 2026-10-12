//! Спецификация N1-06: класс риска инструмента — из реестра в КОДЕ, сетевой гейт — ДО старта.
//!
//! Две дыры пакета проверяются здесь вместе с их отрицаниями: инструмент, объявленный в
//! конфиге репозитория, не получает класс из конфига, а неизвестный не превращается в R0
//! (закон 6); `NetworkPolicy::allows` вызывается как входной гейт, до того как стартует
//! процесс, а не после. Гейт по входу — не вторая блокировка egress: `bwrap` уже несёт
//! `--unshare-net`; здесь проверяется лишь то, что прогону разрешено ПОПЫТАТЬСЯ.

use std::path::{Path, PathBuf};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, NetworkPolicy, RetentionClass, RiskClass};
use stellwerk_gateway::permission::{
    permission_for, Permission, PermissionError, ToolRisk, RISK_PERMISSION,
};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{
    class_covered_in, network_gate, risk_for, NetworkVerdict, SandboxWorker, TOOL_REGISTRY,
};
use stellwerk_task::runner::{Call, ExecOutcome, Verdict, Worker, WorkerError};

const WINDOW: Duration = Duration::from_secs(60);
const BUDGET: Duration = Duration::from_secs(30);
const FIXTURE: &str = "../stellwerk-cli/examples/pi-fixture.sh";
const CLASSES: [RiskClass; 6] = [
    RiskClass::R0,
    RiskClass::R1,
    RiskClass::R2,
    RiskClass::R3,
    RiskClass::R4,
    RiskClass::R5,
];

/// Конфиг репозитория: `polyforge` объявлен R0 (реестр скажет R3), а два модуля с одним
/// и тем же инструментом объявлены с разницей в пять уровней (R0 против R5).
const CONFIG: &str = r#"
[registry]
context_budget_tokens = 1
autoload_budget_tokens = 1
lazy_budget_tokens = 1

[registry.calibration]
tokens_per_byte = 1.0
measured_at = "2026-10-12T00:00:00Z"

[module.polyforge]
kind = "mcp"
risk = "R0"
source = { git = "https://example.com/polyforge", rev = "v1" }
tools = ["gate_report", "gate_evaluate", "evidence_append", "evidence_verify"]

[module.low]
kind = "mcp"
risk = "R0"
source = { git = "https://example.com/low", rev = "v1" }
tools = ["haunt"]

[module.high]
kind = "mcp"
risk = "R5"
source = { git = "https://example.com/high", rev = "v1" }
tools = ["haunt"]
"#;

fn system_dirs() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

fn in_system(names: &[&str]) -> PathBuf {
    names
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("нет ни одного из {names:?}"))
}

fn sh() -> PathBuf {
    in_system(&["/usr/bin/sh", "/bin/sh"])
}

fn echo() -> PathBuf {
    in_system(&["/usr/bin/echo", "/bin/echo"])
}

fn fixture_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sw-n1-06-risk-{tag}-{}", std::process::id()))
}

fn call(tool: &str, module: &str) -> Call {
    Call {
        tool: tool.to_string(),
        module: module.to_string(),
        // Самодекларация из M4: гейт обязан её ИГНОРИРОВАТЬ и брать класс из реестра.
        risk: ToolRisk::Classified(RiskClass::R0),
        data: DataClass::Public,
        model: ModelBinding {
            id: "fixture".to_string(),
            provider: "local".to_string(),
            model_id: "fixture".to_string(),
            data_class: DataClass::Public,
            retention: RetentionClass::FirstParty,
            context_window: 1_000,
            max_output: 1_000,
            budget_tokens_per_task: 1_000,
        },
        interactive: false,
    }
}

fn worker(root: &Path, program: PathBuf, args: Vec<String>) -> SandboxWorker<FixtureEngine> {
    let sandbox = Sandbox {
        work: root.join("work"),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    };
    std::fs::create_dir_all(&sandbox.work).expect("work");
    std::fs::create_dir_all(&sandbox.tmp).expect("tmp");
    SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        program,
        args,
        BUDGET,
        Vec::new(),
    )
}

fn run(worker: &SandboxWorker<FixtureEngine>, call: &Call) -> Result<ExecOutcome, WorkerError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(worker.exec("workspace", call))
}

fn declared_risk(doc: &toml::Value, module: &str) -> String {
    doc["module"][module]["risk"]
        .as_str()
        .unwrap_or_else(|| panic!("у модуля {module} нет risk"))
        .to_string()
}

#[test]
fn orchestrator_declared_r0_but_absent_from_registry_is_unclassified_not_r0() {
    let doc: toml::Value = toml::from_str(CONFIG).expect("конфиг разбирается");
    assert_eq!(declared_risk(&doc, "low"), "R0", "конфиг объявляет R0");

    let risk = risk_for("haunt", "low").expect("неизвестный инструмент — не ошибка");
    assert_eq!(risk, ToolRisk::Unclassified);
    assert_ne!(
        risk,
        ToolRisk::Classified(RiskClass::R0),
        "самодекларация R0 не должна становиться классом"
    );
}

#[test]
fn orchestrator_known_tool_takes_class_from_registry() {
    let risk = risk_for("evidence_append", "polyforge").expect("инструмент есть в реестре");
    assert_eq!(risk, ToolRisk::Classified(RiskClass::R1));
    assert!(
        permission_for(risk).is_ok(),
        "класс реестра обязан подтверждаться таблицей вердиктов"
    );
    for entry in TOOL_REGISTRY {
        assert!(
            permission_for(ToolRisk::Classified(entry.class)).is_ok(),
            "класс {entry:?} не покрыт таблицей вердиктов"
        );
    }
}

#[test]
fn orchestrator_class_absent_from_permission_table_is_table_incomplete() {
    let empty: &[(RiskClass, Permission)] = &[];
    let err = class_covered_in(RiskClass::R5, empty).expect_err("пустая таблица не покрывает R5");
    assert!(matches!(
        err,
        PermissionError::TableIncomplete {
            risk: RiskClass::R5
        }
    ));
    for class in CLASSES {
        assert_eq!(
            class_covered_in(class, &RISK_PERMISSION).is_ok(),
            permission_for(ToolRisk::Classified(class)).is_ok(),
            "полнота реестра и lookup gateway разошлись на {class:?}"
        );
    }
}

#[test]
fn orchestrator_deny_rejects_the_empty_string_url() {
    // Инвариант контракта: Deny запрещает ЛЮБОЙ url, включая пустую строку.
    assert!(network_gate(&NetworkPolicy::Deny, "").is_err());
    assert!(network_gate(&NetworkPolicy::Deny, "https://example.com").is_err());
    // Пустая строка — адрес, а не «выхода нет»: Allow её пропускает, AllowList — нет.
    assert!(network_gate(&NetworkPolicy::Allow, "").is_ok());
    let list = NetworkPolicy::AllowList(vec!["example.com".to_string()]);
    assert!(network_gate(&list, "https://example.com/x?y#z").is_ok());
    assert!(network_gate(&list, "example.com/x").is_err());
    assert!(network_gate(&list, "https://other.example.com/").is_err());
}

#[test]
fn orchestrator_gate_refuses_before_spawn() {
    let root = fixture_root("before-spawn");
    let worker = worker(&root, echo(), vec!["hi".to_string()])
        .with_network(NetworkPolicy::Deny, "https://example.com");

    let refused = run(&worker, &call("haunt", "low"));
    assert!(refused.is_err(), "Deny обязан отказать");
    assert_eq!(
        worker.spawned(),
        0,
        "отказанный прогон не стартует процесс — гейт стоит ДО spawn"
    );
    assert!(
        worker.last_gate().is_none(),
        "отказанный прогон не имеет записи"
    );
    assert!(
        worker.last_gate_refusal().is_some(),
        "причина отказа обязана быть"
    );
}

#[test]
fn orchestrator_allowed_network_passes_gate_and_spawns() {
    let root = fixture_root("allowed");
    let worker = worker(&root, echo(), vec!["hi".to_string()])
        .with_network(NetworkPolicy::Allow, "https://example.com");

    run(&worker, &call("evidence_append", "polyforge")).expect("Allow пропускает запуск");
    assert_eq!(worker.spawned(), 1);
    let record = worker.last_gate().expect("гейт снят до старта");
    assert_eq!(record.risk, ToolRisk::Classified(RiskClass::R1));
    assert_eq!(
        record.network,
        NetworkVerdict::Allowed("https://example.com".to_string())
    );
}

#[test]
fn orchestrator_config_risk_has_no_influence_on_classification() {
    let doc: toml::Value = toml::from_str(CONFIG).expect("конфиг разбирается");
    let low = declared_risk(&doc, "low");
    let high = declared_risk(&doc, "high");
    assert_ne!(low, high, "конфиг объявляет РАЗНЫЕ риски");

    assert_eq!(
        risk_for("haunt", "low").expect("неизвестный инструмент"),
        risk_for("haunt", "high").expect("неизвестный инструмент"),
        "разница в конфиге не должна менять классификацию"
    );

    assert_eq!(declared_risk(&doc, "polyforge"), "R0");
    assert_eq!(
        risk_for("evidence_verify", "polyforge").expect("инструмент есть в реестре"),
        ToolRisk::Classified(RiskClass::R3),
        "класс берётся из реестра, а не из конфига"
    );
}

#[test]
fn orchestrator_successful_run_is_measurable_after_the_gate() {
    let root = fixture_root("measure");
    let work = root.join("work");
    let sandbox = Sandbox {
        work: work.clone(),
        tmp: root.join("tmp"),
        read_only: system_dirs(),
        window: WINDOW,
    };
    std::fs::create_dir_all(&sandbox.work).expect("work");
    std::fs::create_dir_all(&sandbox.tmp).expect("tmp");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::copy(&src, work.join("pi-fixture.sh")).expect("фикстура скопирована");

    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        sh(),
        vec!["pi-fixture.sh".to_string()],
        BUDGET,
        Vec::new(),
    );
    let _ = run(&worker, &call("gate_report", "polyforge")).expect("запуск состоялся");
    let outcome = worker.last_outcome().expect("поток обязан быть измерен");
    assert_eq!(outcome.verdict, Verdict::Valid);
    assert!(worker.last_gate().is_some(), "гейт снят до старта");
}
