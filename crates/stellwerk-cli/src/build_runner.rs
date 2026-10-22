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

use stellwerk_core::contract::{DataClass, ModelBinding, PolicyLevel, RiskClass};
use stellwerk_gateway::gate::CallRequest;
use stellwerk_gateway::permission::ToolRisk;
use stellwerk_task::m5::gate_channel::{GateChannel, GatePolicy, GateServed};
use stellwerk_task::m5::model_transport::{ModelBackend, ModelProxy, Served};
use stellwerk_task::m5::session_identity::{Adjudication, SessionDescription};
use stellwerk_task::orchestrator::{StaticRegistry, ToolEntry};

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
