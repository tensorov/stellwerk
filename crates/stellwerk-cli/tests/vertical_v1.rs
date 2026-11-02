//! V1/v01: сквозное доказательство host-executed edit.
//!
//! Успех, собранный из ответа Pi, не отличает исполнение хостом от исполнения
//! внутри расширения. Поэтому проверяется вся цепочка: Pi просит `edit` → гейт
//! авторизует вызов с digest → хост меняет файл в workspace → digest после записи
//! совпадает с авторизованным → кандидат содержит этот файл с тем же digest.
//! Pi настоящий (1.0.2), backend скриптован: подмена Pi фикстурой спрятала бы
//! именно тот путь, который доказывается.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::json;
use stellwerk_cli::build_runner::{run_build, BuildRun, CandidateFile};
use stellwerk_cli::plan::BuildRequest;
use stellwerk_gateway::gate::ContentDigest;
use stellwerk_task::m5::gate_channel::GateAnswer;
use stellwerk_task::m5::model_transport::{
    BackendError, Frame, ModelBackend, TurnMessage, TurnRequest,
};

const BEFORE: &str = "pub fn one() -> u8 { 1 }\n";
const AFTER: &str = "pub fn one() -> u8 { 2 }\n";

/// Скриптованный ход модели: один `edit` с реальным путём и правкой, затем
/// терминальный текст. Пустая очередь — «агент закончил», а не отказ прогона.
struct Scripted(Mutex<VecDeque<Vec<Frame>>>);

impl ModelBackend for Scripted {
    fn turn(&self, _messages: &[TurnMessage]) -> Result<Vec<Frame>, BackendError> {
        let mut queue = self.0.lock().expect("очередь");
        Ok(queue.pop_front().unwrap_or_else(|| {
            vec![Frame::Text {
                text: "готово".to_string(),
            }]
        }))
    }

    // Build предлагает read/edit/write: сценарий принимает ход вместе с набором и
    // требует, чтобы `edit` действительно был предложен, а не подразумевался.
    fn turn_request(&self, request: &TurnRequest) -> Result<Vec<Frame>, BackendError> {
        let tools = request.tools.as_ref().expect("набор инструментов объявлен");
        assert!(
            tools.iter().any(|tool| tool.name == "edit"),
            "edit обязан быть предложен модели: {:?}",
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>()
        );
        self.turn(&request.messages)
    }
}

fn request() -> BuildRequest {
    BuildRequest {
        goal: "заменить единицу двойкой".to_string(),
        allowed_changes: vec!["src/lib.rs".to_string()],
        acceptance: vec!["тест проходит".to_string()],
        checks: vec!["cargo test".to_string()],
    }
}

fn pi() -> PathBuf {
    let home = std::env::var_os("HOME").expect("HOME задан");
    let launcher = std::env::var_os("PI_BIN").map_or_else(
        || PathBuf::from(home).join(".pi/agent/bin/pi"),
        PathBuf::from,
    );
    assert!(
        launcher.is_file(),
        "нет {}; задайте PI_BIN",
        launcher.display()
    );
    launcher
}

fn extension() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../stellwerk-task/pi/stellwerk_transport.ts")
}

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sw-v01-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("каталог пробега");
    root
}

fn repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect(".git");
    std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    std::fs::write(repo.join("src/lib.rs"), BEFORE).expect("lib.rs");
    repo
}

/// Настоящий Pi просит `edit`; хост авторизует его с digest и применяет запись сам.
/// Проверка идёт по фактическим байтам workspace и кандидата, а не по ответу агента:
/// запись, сделанная внутри расширения, оставила бы файл тем же, но доказательства
/// хостового исполнения не было бы.
#[test]
fn host_executed_edit_end_to_end() {
    let root = scratch("build");
    let repo = repo(&root);
    let call = Frame::ToolCall {
        id: "call-1".to_string(),
        name: "edit".to_string(),
        input: json!({
            "path": "src/lib.rs",
            "edits": [{ "oldText": BEFORE, "newText": AFTER }],
        }),
    };
    let backend = Scripted(Mutex::new(VecDeque::from([vec![call]])));
    let outcome = run_build(
        BuildRun {
            request: &request(),
            repo: &repo,
            root: &root,
            extension: &extension(),
            pi: &pi(),
        },
        &backend,
    )
    .expect("прогон");

    let authorized = ContentDigest::of(AFTER.as_bytes());
    let scope = format!("src/lib.rs#sha256:{}", authorized.as_hex());
    assert!(
        outcome.gate.iter().any(|answer| matches!(
            answer,
            GateAnswer::Authorized { tool, scope: got } if tool == "edit" && *got == scope
        )),
        "гейт обязан авторизовать edit с digest: gate={:?} requests={} turns={} output={}",
        outcome.gate,
        outcome.gate_requests,
        outcome.model_turns,
        outcome.output
    );
    assert!(
        outcome.gate_requests > 0,
        "вызов обязан дойти до гейта: gate={:?} requests={} turns={} output={}",
        outcome.gate,
        outcome.gate_requests,
        outcome.model_turns,
        outcome.output
    );

    let work = root.join("work");
    let written = std::fs::read(work.join("src/lib.rs")).expect("файл после записи");
    assert_eq!(written, AFTER.as_bytes(), "файл изменён хостом");
    assert_eq!(
        ContentDigest::of(&written).as_hex(),
        authorized.as_hex(),
        "digest после записи равен авторизованному"
    );
    assert!(
        outcome.candidate.contains(&CandidateFile {
            path: "src/lib.rs".to_string(),
            digest: authorized.as_hex().to_string(),
        }),
        "кандидат содержит изменённый файл с авторизованным digest: {:?}",
        outcome.candidate
    );
}
