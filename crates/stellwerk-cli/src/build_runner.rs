//! Подготовка площадки Build: копия дерева без истории, политика гейта для canary C и
//! слуга каналов, переживающий неизвестное число ходов.
//!
//! Политика C: гейт разрешает только чтение (`read`, R0) и запрещает оболочку (`bash`,
//! R5 — безусловный `Deny` в таблице рисков). Запрет оболочки — это запрет инструмента
//! целиком, а не фильтр команд: фильтра команд в этом срезе нет, и изображать его здесь
//! было бы хуже, чем назвать отсутствующим.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RetentionClass, RiskClass};
use stellwerk_gateway::gate::CallRequest;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{GateChannel, GatePolicy, GateServed};
use stellwerk_task::m5::model_transport::{ModelBackend, ModelProxy, Served};
use stellwerk_task::m5::sandbox_bwrap::Sandbox;
use stellwerk_task::m5::session_identity::{Adjudication, SessionDescription};
use stellwerk_task::m5::FixtureEngine;
use stellwerk_task::orchestrator::{pi_transport_args, SandboxWorker};
use stellwerk_task::orchestrator::{StaticRegistry, ToolEntry};
use stellwerk_task::runner::{Call, Worker};

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
    let mut served = 0;
    while !stop.load(Ordering::SeqCst) {
        if gate.serve_once(session, policy)? != GateServed::NoRequest {
            served += 1;
        }
    }
    Ok(served)
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

/// Что стало известно после прогона. Числа, а не «получилось»: ходов модели и заявок в гейт
/// может быть ноль, и это факт, а не оттенок успеха.
pub struct BuildOutcome {
    pub model_turns: usize,
    pub gate_requests: usize,
    /// Что напечатал агент. Отказ гейта виден здесь же, в его собственном выводе.
    pub output: String,
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

fn prompt(request: &crate::plan::BuildRequest) -> String {
    let mut out = format!("Задача: {}\n", request.goal);
    out.push_str("Допустимые изменения:\n");
    for item in &request.allowed_changes {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
    out.push_str("Проверки:\n");
    for item in &request.checks {
        out.push_str("- ");
        out.push_str(item);
        out.push('\n');
    }
    out
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
        stellwerk_task::m5::session_identity::ToolSet::listed(&["read", "bash"])
            .map_err(|e| format!("набор инструментов: {e}"))?,
        BUILD_WINDOW,
        stellwerk_task::m5::session_identity::CodemodeState::AbsentAfterConfigLoad,
    )
    .map_err(|e| format!("описание сессии: {e}"))?;

    let mut read_only: Vec<PathBuf> = ["/usr", "/bin", "/lib", "/lib64", "/etc"]
        .map(PathBuf::from)
        .to_vec();
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

    let mut args = pi_transport_args(&extension, &["bash", "read"]);
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

    let call = Call {
        tool: "build".to_string(),
        module: BUILD_MODULE.to_string(),
        risk: ToolRisk::Classified(RiskClass::R0),
        data: DataClass::Public,
        model: build_binding(),
        interactive: false,
    };
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| -> Result<BuildOutcome, String> {
        let model_thread = scope.spawn(|| serve_model_until_stopped(&model, backend, &stop));
        let gate_thread = scope.spawn(|| serve_gate_until_stopped(&gate, &held, &policy, &stop));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("runtime: {e}"))?;
        let outcome = runtime
            .block_on(worker.exec("workspace", &call))
            .map_err(|e| format!("запуск pi: {}", e.0))?;
        stop.store(true, Ordering::SeqCst);
        for socket in [model.socket(), gate.socket()] {
            wake(socket).map_err(|e| format!("пробуждение {}: {e}", socket.display()))?;
        }
        let model_turns = model_thread
            .join()
            .map_err(|_| "поток модели упал".to_string())?
            .map_err(|e| format!("канал модели: {e}"))?;
        let gate_requests = gate_thread
            .join()
            .map_err(|_| "поток гейта упал".to_string())?
            .map_err(|e| format!("канал гейта: {e}"))?;
        Ok(BuildOutcome {
            model_turns,
            gate_requests,
            output: outcome.output,
        })
    })
}
