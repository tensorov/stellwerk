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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::m5::sandbox_bwrap::Sandbox;
use crate::m5::EngineAdapter;
use crate::runner::{
    stop_reason_from, verdict_for, Call, ExecOutcome, Progress, RunOutcome, StopFacts, Worker,
    WorkerError,
};

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
    /// Замер последнего запуска. `Worker::exec` возвращает `ExecOutcome` с одним лишь
    /// текстом (сигнатура из M4 не переписывается), а `evaluate()` требует `RunOutcome`;
    /// поэтому измерение живёт здесь и выдаётся через `last_outcome`/`last_refusal`.
    measure: Arc<Mutex<Measure>>,
}

/// Состояние замера последнего запуска. Три РАЗНЫХ состояния, а не «есть/нет»: «замера
/// ещё не было», «поток измерен» и «поток отвергнут» чинятся по-разному. Слить
/// отвергнутый ввод с отсутствием прогона значило бы выдать неразобранное за
/// неслучившееся (закон 6).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Measure {
    /// Воркер ещё не довёл запуск до замера.
    Pending,
    /// Поток разобран: счётчики и вердикт посчитаны.
    Measured(RunOutcome),
    /// Поток отвергнут: строка не является Pi-событием. Причина сохранена.
    Refused(String),
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
            measure: Arc::new(Mutex::new(Measure::Pending)),
        }
    }

    /// Последний измеренный прогон. `None` — замер не состоялся (ещё не запускали или
    /// поток отвергнут); отвергнутая причина видна через `last_refusal`, и это не то же
    /// самое, что пустой поток: у пустого есть `RunOutcome` с `Verdict::InvalidLoop`.
    pub fn last_outcome(&self) -> Option<RunOutcome> {
        match &*self.lock_measure() {
            Measure::Measured(outcome) => Some(*outcome),
            Measure::Pending | Measure::Refused(_) => None,
        }
    }

    /// Почему поток последнего запуска отвергнут. `None` — отказа не было.
    pub fn last_refusal(&self) -> Option<String> {
        match &*self.lock_measure() {
            Measure::Refused(reason) => Some(reason.clone()),
            Measure::Pending | Measure::Measured(_) => None,
        }
    }

    /// Достать замок, переживший панику потока с ним. Отравление — это «другой поток
    /// упал, держа замок», а не «замера не было»: терять уже записанный замер из-за
    /// этого значило бы выдать отсутствие данных за отсутствие прогона (закон 6).
    fn lock_measure(&self) -> std::sync::MutexGuard<'_, Measure> {
        self.measure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Разобрать stdout прогона в измеренный `RunOutcome`.
///
/// Строки идут через `Progress::absorb`, счётчики превращаются в вердикт через
/// `verdict_for`; пороги берутся из раннера (`GATE_REQUIRE_*`), своих чисел здесь нет.
/// Битая строка — отказ с номером и текстом, а не молча пропущенный шаг: потерянный
/// шаг неотличим от прогона без шага (закон 6).
fn measure(stdout: &str) -> Result<RunOutcome, WorkerError> {
    let mut progress = Progress::default();
    for line in stdout.lines() {
        Progress::absorb(&mut progress, line)
            .map_err(|e| WorkerError(format!("worker stream is not a Pi event: {e}")))?;
    }
    // Вызывается только на коде 0: таймаут и ненулевой выход отсекает `session` и ветка
    // ниже, поэтому `timed_out`/`exit_code` здесь — факты чистого завершения.
    let stop_reason = stop_reason_from(StopFacts {
        timed_out: false,
        exit_code: 0,
        named: progress.stop_reason,
    });
    Ok(RunOutcome {
        stop_reason,
        tool_steps: progress.tool_steps,
        turns: progress.turns,
        total_input_tokens: progress.total_input_tokens,
        verdict: verdict_for(stop_reason, progress.tool_steps, progress.turns),
    })
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
                Some(0) => {
                    // Код 0 — факт процесса, а не годность прогона: измеряем поток,
                    // чтобы вызывающий увидел `RunOutcome`, а не голый текст.
                    match measure(&outcome.stdout) {
                        Ok(measured) => *self.lock_measure() = Measure::Measured(measured),
                        Err(WorkerError(reason)) => *self.lock_measure() = Measure::Refused(reason),
                    }
                    Ok(ExecOutcome {
                        output: outcome.stdout,
                    })
                }
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
