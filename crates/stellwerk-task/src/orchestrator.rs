//! # Оркестратор запуска: единственный production-путь к движку
//!
//! Здесь сходятся три вещи, которые до N1-03 жили порознь: песочница собирает
//! `bwrap`-команду, окружение очищается и наполняется только разрешёнными
//! переменными, а исполняет всё это адаптер. Порядок обязателен: `Sandbox::command`
//! вызывается ДО `EngineAdapter::session`, и результат передаётся в него. Адаптер
//! не собирает команду сам — иначе `bwrap` и `env_clear` остаются декорацией, а не
//! гарантией (ADR-0016).
//!
//! Второго production-пути запуска нет: `SandboxWorker` — единственная реализация
//! `Worker` в этом крейте, и она не собирает команду сама ни в одной ветке.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::m5::sandbox_bwrap::Sandbox;
use crate::m5::EngineAdapter;
use crate::runner::{Call, ExecOutcome, Worker, WorkerError};

/// Воркер, запускающий движок ВНУТРИ песочницы через адаптер.
///
/// Поля закрыты: программу, аргументы, бюджет и разрешённые переменные задаёт
/// вызывающий при сборке, а не окружение процесса. Один и тот же воркер обязан
/// давать один и тот же запуск в любом терминале.
pub struct SandboxWorker<A> {
    adapter: A,
    sandbox: Sandbox,
    program: PathBuf,
    args: Vec<String>,
    budget: Duration,
    env: Vec<(String, String)>,
    spawned: AtomicUsize,
}

impl<A> SandboxWorker<A> {
    /// Собрать воркер. `env` — ЯВНЫЙ набор переменных: после `env_clear` в окружении
    /// агента не остаётся ничего, кроме перечисленного здесь. Пустой список — это
    /// «переменных нет», а не «переменные не заданы».
    pub fn new(
        adapter: A,
        sandbox: Sandbox,
        program: impl Into<PathBuf>,
        args: Vec<String>,
        budget: Duration,
        env: Vec<(String, String)>,
    ) -> Self {
        Self {
            adapter,
            sandbox,
            program: program.into(),
            args,
            budget,
            env,
            spawned: AtomicUsize::new(0),
        }
    }
}

impl<A: EngineAdapter + Sync> Worker for SandboxWorker<A> {
    fn exec(
        &self,
        _workspace_ref: &str,
        _call: &Call,
    ) -> Pin<Box<dyn Future<Output = Result<ExecOutcome, WorkerError>> + Send + '_>> {
        Box::pin(async move {
            self.spawned.fetch_add(1, Ordering::SeqCst);
            // Подготовленный запуск: `bwrap`-argv собирает песочница, а не адаптер.
            let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
            let mut launch = self
                .sandbox
                .command(&self.sandbox.work, &self.program, &args, self.budget)
                .map_err(|e| WorkerError(format!("sandbox refused the run: {e}")))?;
            // Окружение задаётся ДО spawn: пустое, затем только разрешённые переменные.
            launch.env_clear();
            for (key, value) in &self.env {
                launch.env(key, value);
            }
            let outcome = self
                .adapter
                .session(launch, self.budget)
                .map_err(|e| WorkerError(format!("engine session failed: {e}")))?;
            // Ненулевой код и убийство сигналом — РАЗНЫЕ отказы, и ни один не успех:
            // иначе упавший прогон отчитался бы `Succeeded` (закон 6).
            match outcome.code {
                Some(0) => Ok(ExecOutcome {
                    output: outcome.stdout,
                }),
                Some(code) => Err(WorkerError(format!(
                    "process exited with code {code}: {}",
                    outcome.stderr.trim()
                ))),
                None => Err(WorkerError(format!(
                    "process was killed by a signal: {}",
                    outcome.stderr.trim()
                ))),
            }
        })
    }

    fn spawned(&self) -> usize {
        self.spawned.load(Ordering::SeqCst)
    }
}
