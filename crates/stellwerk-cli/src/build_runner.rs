//! Подготовка площадки Build: копия дерева без истории, политика гейта для canary C и
//! слуга каналов, переживающий неизвестное число ходов.
//!
//! Политика записи: запрет определяется уровнем доступа, а не именем инструмента.
//! `edit`/`write` — R1 (правка внутри рабочего места, откат удалением одноразовой копии),
//! поэтому таблица рисков их разрешает; `bash` — R5, и он запрещён целиком, а не фильтром
//! команд: фильтра команд в этом срезе нет, и изображать его было бы хуже, чем назвать
//! отсутствующим.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::{writes_content, AuthorizedCall, CallRequest, ContentDigest};
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_store::SqliteStore;
use stellwerk_task::m5::gate_channel::{
    AllowBasis, GateAnswer, GateChannel, GateDecision, GatePolicy, GateServed, PolicyVerdict,
};
use stellwerk_task::m5::model_transport::{ModelBackend, ModelProxy, Served};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::session_identity::{Adjudication, SessionDescription};
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{pi_transport_args, SandboxWorker};
use stellwerk_task::orchestrator::{StaticRegistry, ToolEntry};
use stellwerk_task::runner::{
    Call, CallAttempt, CallVerdict, ExecOutcome, RiskRegistry, Worker, WorkerError,
};

/// Модуль, от имени которого гейт видит инструменты агента.
pub const BUILD_MODULE: &str = "stellwerk-build";

/// Доверенный реестр для canary C. Класс задаётся здесь, а не вызовом: агент не вправе
/// назвать себя безопасным.
pub const BUILD_TOOLS: &[ToolEntry] = &[
    ToolEntry {
        module: BUILD_MODULE,
        tool: "read",
        class: RiskClass::R0,
    },
    // R1 — запись в пределы рабочего места: правка файлов в песочнице, откат удалением
    // копии. Класс задан здесь, а не вызовом: агент не вправе назвать себя безопасным.
    ToolEntry {
        module: BUILD_MODULE,
        tool: "edit",
        class: RiskClass::R1,
    },
    ToolEntry {
        module: BUILD_MODULE,
        tool: "write",
        class: RiskClass::R1,
    },
    // R5 — единственный класс с безусловным `Deny` при любом уровне, включая владельца.
    // Для canary это и нужно: оболочка не авторизуется вообще, а не «при некоторых условиях».
    ToolEntry {
        module: BUILD_MODULE,
        tool: "bash",
        class: RiskClass::R5,
    },
    // Внешний вызов прогона. Гейт его не видит: агент не может назвать инструмент, которого
    // нет в его наборе, а «build» в наборе нет.
    ToolEntry {
        module: BUILD_MODULE,
        tool: "build",
        class: RiskClass::R0,
    },
];

pub const BUILD_REGISTRY: StaticRegistry = StaticRegistry(BUILD_TOOLS);

/// Что модель видит в `--tools`. Оболочки здесь нет: предлагать инструмент, который
/// политика заведомо отклонит, — это обещание, которого прогон не выполнит.
pub const BUILD_OFFERED: &[&str] = &["read", "edit", "write"];

/// Что гейт держит в наборе сессии. `bash` остаётся здесь, хотя модели не предлагается:
/// R5 — это отказ по уровню, а не отсутствие инструмента, и эти два состояния должны
/// быть различимы в решении гейта.
pub const BUILD_HELD: &[&str] = &["read", "bash", "edit", "write"];

/// Копия дерева для песочницы. `.git` не копируется: `Sandbox::preflight` отвергает
/// историю внутри `work`, и это единственный способ её там не иметь — не удалять после,
/// а не класть.
pub fn copy_workspace(repo: &Path, dest: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dest)?;
    copy_entries(repo, dest)
}

fn copy_entries(from: &Path, to: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let src: PathBuf = entry.path();
        let dst = to.join(&name);
        let meta = std::fs::symlink_metadata(&src)?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            // Ссылка остаётся ссылкой: копирование «за ней» внесло бы в песочницу
            // содержимое, которого в дереве нет, а внутри она указывает на смонтированное
            // только на чтение — или никуда.
            std::os::unix::fs::symlink(std::fs::read_link(&src)?, &dst)?;
        } else if kind.is_dir() {
            std::fs::create_dir(&dst)?;
            copy_entries(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Политика гейта для canary C. Возвращает `Some` всегда: `Unclassified` — это тоже
/// ответ (вопрос к реестру), и `None` («никто не ответил») здесь был бы подменой.
pub struct BuildPolicy {
    pub model: ModelBinding,
}

impl GatePolicy for BuildPolicy {
    fn request_for(&self, call: &Adjudication) -> Option<CallRequest> {
        let risk = BUILD_TOOLS
            .iter()
            .find(|entry| entry.tool == call.tool && entry.module == BUILD_MODULE)
            .map_or(ToolRisk::Unclassified, |entry| {
                ToolRisk::Classified(entry.class)
            });
        Some(CallRequest {
            tool: call.tool.clone(),
            module: BUILD_MODULE.to_string(),
            risk,
            caller: PolicyLevel::L2Contributor,
            data: DataClass::Public,
            model: self.model.clone(),
            // Спросить некого: прогон не интерактивен, и `Ask` превратился бы в «пока
            // подумаем», то есть в разрешение по факту ожидания.
            interactive: false,
        })
    }

    /// Запрет — по уровню доступа, не по пути: `path` едет в кадре как след вызова, но
    /// прав не сужает. Умолчание трейта отказало бы любому пути, поэтому политика,
    /// которая путь принимает, обязана сказать это явно.
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

/// Обслуживает гейт, пока `stop` не поднят.
///
/// `accept` не просыпается сам: остановка — это поднятый флаг **и** пустое подключение
/// (`wake`). Подключение даёт `EOF` → `GateServed::NoRequest`, цикл видит флаг и выходит.
/// Возвращает число обслуженных заявок; ноль — заявок не было, и это отличается от
/// «были и все разрешены».
pub fn serve_gate_until_stopped(
    gate: &GateChannel,
    session: &SessionDescription,
    policy: &dyn GatePolicy,
    stop: &AtomicBool,
) -> io::Result<usize> {
    serve_gate_until_stopped_logged(gate, session, policy, stop).map(|(served, _)| served)
}

/// То же, но с журналом решений: «заявок ноль» и «все разрешены» различимы только
/// вместе с ответами.
pub fn serve_gate_until_stopped_logged(
    gate: &GateChannel,
    session: &SessionDescription,
    policy: &dyn GatePolicy,
    stop: &AtomicBool,
) -> io::Result<(usize, Vec<GateAnswer>)> {
    let mut served = 0;
    let mut answers = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        let (status, decision) = gate.serve_once_logged(session, policy)?;
        if status != GateServed::NoRequest {
            served += 1;
        }
        if let Some(decision) = decision {
            answers.push(decision.into_answer());
        }
    }
    Ok((served, answers))
}

/// Обслуживает канал модели по тому же правилу остановки.
pub fn serve_model_until_stopped(
    model: &ModelProxy,
    backend: &dyn ModelBackend,
    stop: &AtomicBool,
) -> io::Result<usize> {
    let mut served = 0;
    while !stop.load(Ordering::SeqCst) {
        if model.serve_once(backend)? != Served::NoRequest {
            served += 1;
        }
    }
    Ok(served)
}

/// Будит заблокированный `accept`: соединение закрывается сразу, сервер читает `EOF`.
pub fn wake(socket: &Path) -> io::Result<()> {
    std::os::unix::net::UnixStream::connect(socket).map(drop)
}

/// Корни, видимые агенту только на чтение. Токен под ними лежать не может: прочитав его
/// сам, агент обошёл бы прокси. Список один на песочницу и на проверку пути токена.
pub fn sandbox_roots() -> Vec<PathBuf> {
    ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .map(PathBuf::from)
        .to_vec()
}

/// Отказ хостского исполнения. Корень, выход за workspace, несвязанный digest,
/// не-пишущий инструмент и отказ гейта — разные состояния; ни одно не становится
/// тихой записью.
#[derive(Debug)]
pub enum HostRefusal {
    WorkspaceMissing {
        root: PathBuf,
    },
    OutsideWorkspace {
        root: PathBuf,
        path: PathBuf,
    },
    NotWritable {
        tool: String,
    },
    NotAuthorized {
        answer: GateAnswer,
    },
    DigestMismatch {
        stage: &'static str,
        authorized: String,
        actual: String,
    },
    Io {
        path: PathBuf,
        message: String,
    },
}

/// Что хост записал. Digest — фактический digest файла после записи.
#[derive(Debug)]
pub struct HostWrite {
    pub path: PathBuf,
    pub digest: String,
    pub bytes: usize,
}

fn io_refusal(path: &Path, error: io::Error) -> HostRefusal {
    HostRefusal::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

static HOST_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Применяет решение гейта. Пишет только `authorized()`: у `Allow` и прочих ответов
/// нет `AuthorizedCall`, поэтому применить их как запись нельзя — это отказ.
pub fn apply_gate_decision(root: &Path, decision: &GateDecision) -> Result<HostWrite, HostRefusal> {
    let call = decision
        .authorized()
        .ok_or_else(|| HostRefusal::NotAuthorized {
            answer: decision.answer().clone(),
        })?;
    apply_authorized(root, call)
}

fn apply_authorized(root: &Path, call: &AuthorizedCall) -> Result<HostWrite, HostRefusal> {
    if !writes_content(call.tool()) {
        return Err(HostRefusal::NotWritable {
            tool: call.tool().to_owned(),
        });
    }
    let target = resolve_in_workspace(root, call.path())?;
    // До записи: байты, которые собираемся писать, — ровно авторизованные.
    let actual = ContentDigest::of(call.argument());
    if actual != *call.content_digest() {
        return Err(HostRefusal::DigestMismatch {
            stage: "authorized",
            authorized: call.content_digest().as_hex().to_owned(),
            actual: actual.as_hex().to_owned(),
        });
    }
    let Some(parent) = target.parent() else {
        return Err(HostRefusal::OutsideWorkspace {
            root: root.to_path_buf(),
            path: target,
        });
    };
    let name = target
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let temp = parent.join(format!(
        ".{name}.host-{}-{}",
        std::process::id(),
        HOST_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    // После записи (ещё во временном файле): то, что появится под целевым именем,
    // обязано иметь авторизованный digest. Иначе цель не подменяется вообще.
    let staged = std::fs::write(&temp, call.argument())
        .and_then(|()| std::fs::read(&temp))
        .map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            io_refusal(&temp, error)
        })?;
    let staged_digest = ContentDigest::of(&staged);
    if staged_digest != *call.content_digest() {
        let _ = std::fs::remove_file(&temp);
        return Err(HostRefusal::DigestMismatch {
            stage: "staged",
            authorized: call.content_digest().as_hex().to_owned(),
            actual: staged_digest.as_hex().to_owned(),
        });
    }
    std::fs::rename(&temp, &target).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        io_refusal(&target, error)
    })?;
    let written = std::fs::read(&target).map_err(|error| io_refusal(&target, error))?;
    let digest = ContentDigest::of(&written);
    if digest != *call.content_digest() {
        return Err(HostRefusal::DigestMismatch {
            stage: "written",
            authorized: call.content_digest().as_hex().to_owned(),
            actual: digest.as_hex().to_owned(),
        });
    }
    Ok(HostWrite {
        path: target,
        digest: digest.as_hex().to_owned(),
        bytes: written.len(),
    })
}

/// Разрешает путь строго внутри `root`. Корень канонизируется: его отсутствие —
/// названный отказ. `..`, абсолютный путь вне корня и симлинк наружу отвергаются
/// до записи.
pub fn resolve_in_workspace(root: &Path, raw: &str) -> Result<PathBuf, HostRefusal> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            HostRefusal::WorkspaceMissing {
                root: root.to_path_buf(),
            }
        } else {
            io_refusal(root, error)
        }
    })?;
    let outside = |path: PathBuf| HostRefusal::OutsideWorkspace {
        root: root.clone(),
        path,
    };
    let requested = Path::new(raw);
    let relative = if requested.is_absolute() {
        requested
            .strip_prefix(&root)
            .map_err(|_| outside(requested.to_path_buf()))?
            .to_path_buf()
    } else {
        requested.to_path_buf()
    };
    if relative.as_os_str().is_empty()
        || relative.components().any(|part| {
            !matches!(
                part,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(outside(requested.to_path_buf()));
    }
    let candidate = root.join(&relative);
    let parent = std::fs::canonicalize(candidate.parent().unwrap_or(&root))
        .map_err(|error| io_refusal(&root, error))?;
    if !parent.starts_with(&root) {
        return Err(outside(candidate));
    }
    let Some(name) = candidate.file_name() else {
        return Err(outside(candidate));
    };
    let target = parent.join(name);
    // Симлинк разрешается только внутрь корня: иначе запись ушла бы за workspace,
    // хотя лексически путь внутри.
    if let Ok(meta) = std::fs::symlink_metadata(&target) {
        if meta.file_type().is_symlink() {
            let real =
                std::fs::canonicalize(&target).map_err(|error| io_refusal(&target, error))?;
            if !real.starts_with(&root) {
                return Err(outside(target));
            }
        }
    }
    Ok(target)
}

/// Сколько ждёт песочница и сколько отведено прогону.
pub const BUILD_WINDOW: Duration = Duration::from_secs(60);
pub const BUILD_BUDGET: Duration = Duration::from_secs(30);

/// Что именно исполняется: план, дерево, корень для `work`/`tmp`/`sock` и расширение Pi.
/// Расширение приходит путём, а не константой: путь к нему — забота вызывающего, а не
/// модуля, который не должен угадывать раскладку крейтов.
pub struct BuildRun<'a> {
    pub request: &'a crate::plan::BuildRequest,
    pub repo: &'a Path,
    pub root: &'a Path,
    pub extension: &'a Path,
    pub pi: &'a Path,
}

/// Идентичность Build: task-файла нет, но есть дерево, которое собирают. Идентификатор —
/// функция от репозитория, а не случайный id, поэтому повторный прогон того же дерева
/// остаётся тем же прогоном.
pub fn build_task_id(repo: &Path) -> String {
    format!("build:{}", repo.display())
}

/// Чек Build лежит внутри его `run.root`: путь выводится из данных прогона и удаляется
/// вместе с ним, а не оседает в общем каталоге.
pub fn build_receipt_store(root: &Path) -> PathBuf {
    root.join("receipts.db")
}

/// Обёртка воркера, запоминающая вывод: `run_once` пишет в чек только хеш, а Build
/// показывает ответ агента. Решение по-прежнему выносит `run_once`, поэтому здесь нет
/// второго пути авторизации — только сохранение результата запуска.
struct OutputRecording<'a, W: Worker + Sync> {
    inner: &'a W,
    output: Mutex<String>,
}

impl<W: Worker + Sync> Worker for OutputRecording<'_, W> {
    fn exec(
        &self,
        workspace_ref: &str,
        call: &Call,
    ) -> Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + '_>> {
        let inner = self.inner;
        let output = &self.output;
        let workspace = workspace_ref.to_string();
        let call = call.clone();
        Box::pin(async move {
            let outcome = inner.exec(&workspace, &call).await?;
            *output.lock().expect("output mutex poisoned") = outcome.output.clone();
            Ok(outcome)
        })
    }

    fn spawned(&self) -> usize {
        self.inner.spawned()
    }

    fn registry(&self) -> &dyn RiskRegistry {
        self.inner.registry()
    }
}

/// Что стало известно после прогона. Числа, а не «получилось»: ходов модели и заявок в гейт
/// может быть ноль, и это факт, а не оттенок успеха.
pub struct BuildOutcome {
    pub model_turns: usize,
    pub gate_requests: usize,
    /// Сами решения гейта: имя инструмента и основание разрешения либо причина отказа.
    pub gate: Vec<GateAnswer>,
    /// Что напечатал агент. Отказ гейта виден здесь же, в его собственном выводе.
    pub output: String,
    /// Решение общего пути исполнения по внешнему вызову Build. Несёт `Allow`/`Deny`/
    /// `Suspended`/`Ask` из единственного `authorize`; `Deny`/`Suspended`/`Ask` при этом
    /// записаны в чек как `status=Denied|Suspended|Asked` с причиной. Отличие отказа от
    /// «агент ничего не сделал» сегодня видно в чеке, а не на экране: plan TUI поле пока
    /// не читает, и его показ — отдельный follow-up.
    pub decision: CallVerdict,
}

/// Краткая запись решений гейта: что разрешено и на каком основании, что отклонено и
/// почему. Ноль решений — это «решений нет», а не пустая строка.
pub fn gate_summary(answers: &[GateAnswer]) -> String {
    if answers.is_empty() {
        return "решений нет".to_string();
    }
    answers
        .iter()
        .map(|answer| match answer {
            GateAnswer::Allow { tool } => format!("allowed({tool})"),
            GateAnswer::Authorized { tool, scope } => format!("authorized({tool}:{scope})"),
            GateAnswer::Deny { tool, reason } => format!("denied({tool}:{reason})"),
            GateAnswer::Ask { tool } => format!("asked({tool})"),
            GateAnswer::Suspended { tool } => format!("suspended({tool})"),
            GateAnswer::Unclassified { tool } => format!("unclassified({tool})"),
            GateAnswer::UnknownTool { tool, .. } => format!("unknown_tool({tool})"),
            GateAnswer::UnrecognisedSource {
                parent_tool_call_id,
            } => format!("unrecognised({parent_tool_call_id})"),
            GateAnswer::Refused { code, .. } => format!("refused({code:?})"),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Что сказала модель. Поток `--mode json` — построчный JSON, и первые 240 символов
/// этого потока всегда заголовок сессии; печатать их значило показывать не ответ.
/// Если ответа в потоке нет, возвращается начало вывода: пустая строка скрыла бы,
/// что печатать было что.
pub fn answer_text(output: &str) -> String {
    let mut last = String::new();
    for line in output.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        collect_assistant(&value, &mut last);
    }
    if last.is_empty() {
        output.chars().take(240).collect()
    } else {
        last
    }
}

fn collect_assistant(value: &serde_json::Value, last: &mut String) {
    if value.get("role").and_then(serde_json::Value::as_str) == Some("assistant") {
        if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
            *last = text.to_string();
            return;
        }
        if let Some(blocks) = value.get("content").and_then(serde_json::Value::as_array) {
            let text: String = blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(serde_json::Value::as_str))
                .collect();
            if !text.is_empty() {
                *last = text;
                return;
            }
        }
    }
    for nested in value.as_object().into_iter().flat_map(|o| o.values()) {
        collect_assistant(nested, last);
    }
}

fn build_binding() -> ModelBinding {
    ModelBinding {
        id: "build".to_string(),
        provider: "neuraldeep".to_string(),
        model_id: crate::plan::HOST_MODEL_ID.to_string(),
        data_class: DataClass::Public,
        retention: RetentionClass::FirstParty,
        context_window: 138_336,
        max_output: 41_500,
        budget_tokens_per_task: 100_000,
    }
}

pub fn prompt(request: &crate::plan::BuildRequest) -> String {
    let mut out = format!("Задача: {}\n\n", request.goal);
    // Модель без этой строки пересказывает план и завершает ход: заявок в гейт ноль,
    // и «разрешено» неотличимо от «никто не просил».
    out.push_str("Не пересказывай план — выполняй его. Правь файлы инструментом edit.\n");
    out.push_str("Оболочка запрещена политикой; доступны read, edit, write.\n\n");
    out.push_str("Допустимые изменения:\n");
    for item in &request.allowed_changes {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
    out.push_str("Проверки (их выполнит человек, не запускай их сам):\n");
    for item in &request.checks {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
    out
}

/// Аргументы запуска Pi для Build: предлагаемый набор — [`BUILD_OFFERED`], а не набор
/// гейта. Публичная, потому что контракт «что предложено» проверяется из `tests/`.
pub fn build_args(extension: &Path) -> Vec<String> {
    pi_transport_args(extension, BUILD_OFFERED)
}

/// Один прогон: копия дерева, `pi` внутри `bwrap`, наши каналы и политика C.
///
/// Возвращает [`BuildOutcome`] и тогда, когда агент ничего не сделал: отказ гейта виден в
/// его собственном выводе (`gate_denied`), а не превращается здесь в «прогон не удался».
pub fn run_build(run: BuildRun<'_>, backend: &dyn ModelBackend) -> Result<BuildOutcome, String> {
    let (work, tmp, sock) = (
        run.root.join("work"),
        run.root.join("tmp"),
        run.root.join("sock"),
    );
    for dir in [&work, &tmp, &sock] {
        std::fs::create_dir_all(dir).map_err(|e| format!("каталог {}: {e}", dir.display()))?;
    }
    copy_workspace(run.repo, &work).map_err(|e| format!("копия дерева: {e}"))?;
    let extension = work.join("stellwerk_transport.ts");
    std::fs::copy(run.extension, &extension)
        .map_err(|e| format!("расширение не скопировано: {e}"))?;

    let model = ModelProxy::bind(&sock).map_err(|e| format!("канал модели: {e}"))?;
    let gate = GateChannel::bind(&sock).map_err(|e| format!("канал гейта: {e}"))?;
    let policy = BuildPolicy {
        model: build_binding(),
    };
    let held = stellwerk_task::m5::session_identity::SessionDescription::new(
        "build",
        &tmp,
        stellwerk_task::m5::session_identity::ToolSet::listed(BUILD_HELD)
            .map_err(|e| format!("набор инструментов: {e}"))?,
        BUILD_WINDOW,
        stellwerk_task::m5::session_identity::CodemodeState::AbsentAfterConfigLoad,
    )
    .map_err(|e| format!("описание сессии: {e}"))?;

    let mut read_only = sandbox_roots();
    if let Some(bin) = run.pi.parent() {
        read_only.push(bin.to_path_buf());
        if let Some(agent) = bin.parent() {
            read_only.push(agent.join("install"));
        }
    }
    let sandbox = Sandbox {
        work: work.clone(),
        tmp: tmp.clone(),
        read_only,
        window: BUILD_WINDOW,
    };

    let mut args = build_args(&extension);
    for tail in [
        "--provider",
        "stellwerk",
        "--model",
        "proxy",
        "--mode",
        "json",
        "-p",
    ] {
        args.push(tail.to_string());
    }
    args.push(prompt(run.request));

    let home = work.join("home");
    std::fs::create_dir_all(&home).map_err(|e| format!("дом агента: {e}"))?;
    let worker = SandboxWorker::new(
        FixtureEngine::new("/bin/true"),
        sandbox,
        run.pi,
        args,
        BUILD_BUDGET,
        vec![
            ("HOME".to_string(), home.display().to_string()),
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("TMPDIR".to_string(), tmp.display().to_string()),
            (
                "STELLWERK_MODEL_SOCK".to_string(),
                model.socket().display().to_string(),
            ),
            (
                "STELLWERK_GATE_SOCK".to_string(),
                gate.socket().display().to_string(),
            ),
        ],
    )
    .with_sockets(model.socket(), gate.socket())
    .map_err(|e| format!("сокеты: {}", e.0))?
    .with_registry(BUILD_REGISTRY);

    let attempt = CallAttempt {
        task_id: build_task_id(run.repo),
        call: Call {
            tool: "build".to_string(),
            module: BUILD_MODULE.to_string(),
            risk: ToolRisk::Classified(RiskClass::R0),
            data: DataClass::Public,
            model: build_binding(),
            interactive: false,
        },
        caller: PolicyLevel::L2Contributor,
        workspace_ref: "workspace".to_string(),
    };
    let store_path = build_receipt_store(run.root);
    let store = SqliteStore::open(&store_path)
        .map_err(|e| format!("чек Build `{}`: {e}", store_path.display()))?;
    let recording = OutputRecording {
        inner: &worker,
        output: Mutex::new(String::new()),
    };
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| -> Result<BuildOutcome, String> {
        let model_thread = scope.spawn(|| serve_model_until_stopped(&model, backend, &stop));
        let gate_thread =
            scope.spawn(|| serve_gate_until_stopped_logged(&gate, &held, &policy, &stop));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("runtime: {e}"))?;
        // `run_once` останавливает оба слуги только после возврата, поэтому остановка и
        // пробуждение выполняются ДО разбора результата: отказ авторизации не должен
        // оставлять потоки в `accept`.
        let result = runtime.block_on(crate::execute(&attempt, &recording, &store));
        stop.store(true, Ordering::SeqCst);
        for socket in [model.socket(), gate.socket()] {
            wake(socket).map_err(|e| format!("пробуждение {}: {e}", socket.display()))?;
        }
        let model_turns = model_thread
            .join()
            .map_err(|_| "поток модели упал".to_string())?
            .map_err(|e| format!("канал модели: {e}"))?;
        let (gate_requests, gate) = gate_thread
            .join()
            .map_err(|_| "поток гейта упал".to_string())?
            .map_err(|e| format!("канал гейта: {e}"))?;
        let decision = result.map_err(|e| format!("запуск pi: {e}"))?;
        let output = recording
            .output
            .lock()
            .map_err(|_| "вывод прогона отравлен".to_string())?
            .clone();
        Ok(BuildOutcome {
            model_turns,
            gate_requests,
            gate,
            output,
            decision,
        })
    })
}
