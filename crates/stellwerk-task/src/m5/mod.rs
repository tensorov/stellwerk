//! # Единый интерфейс запуска сессии агента
//!
//! Движок — единственное место, где stellwerk говорит с внешней программой агента.
//! Песочница и границы доверия проверяются настоящим `bwrap`, а не наличием Pi, поэтому
//! контракт и фикстура строятся так, чтобы проверка границ доверия не требовала Pi.
//!
//! Бюджеты, песочница и одобрение сюда НЕ входят: иначе проверка доступности задела бы
//! политику, которой ещё нет, и фикстура начала бы её выдумывать.

// Песочница живёт отдельным подмодулем: собирает и отвергает, но не запускает.
pub mod sandbox_bwrap;
// Идентичность сессии: каталог, ЯВНЫЙ набор инструментов, лимит, идентификатор.
pub mod session_identity;
// Секреты: окружение агента, аргументы команды и прокси — три места, где токен не появляется.
pub mod secrets;
// Профиль интегратора: перечень видимых путей задан в коде, и пути к чужому секрету нет.
pub mod integrator_profile;
// Транспорт модели: кадры на хостовом сокете, маршрут выбирает хост, а не песочница.
pub mod model_transport;
// Провод: разбор базового адреса, TLS и чтение ответа — байты без политики.
pub mod http_wire;
// Канал гейта: вердикт по инструменту ДО эффекта, риск классифицирует хост.
pub mod gate_channel;
// Ходовой backend: кадр хода уходит в OpenAI-совместимый сервис, сеть есть только у хоста.
pub mod model_backend;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;

/// Отчёт о доступности движка. Причина — поле, а не украшение: «недоступен» без причины
/// при разборе инцидента бесполезен. Поля закрыты — отчёт собирается только через
/// конструкторы, и оба требуют непустой причины.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    engine: String,
    available: bool,
    reason: String,
}

impl ProbeReport {
    /// Доступен — и вот почему: «работает» без объяснения приглушает тот самый случай.
    pub fn available(engine: &str, reason: &str) -> Result<Self, EngineError> {
        Self::build(engine, true, reason)
    }

    /// Недоступен — и вот почему: «нет файла» и «нет права выполнения» суть разные
    /// инциденты, а молчаливый `false` схлопывает их в один.
    pub fn unavailable(engine: &str, reason: &str) -> Result<Self, EngineError> {
        Self::build(engine, false, reason)
    }

    /// Общая дверь: пустая причина — отсутствие данных, а не вердикт, поэтому `Err`.
    fn build(engine: &str, available: bool, reason: &str) -> Result<Self, EngineError> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(EngineError::NoReason {
                engine: engine.to_owned(),
            });
        }
        Ok(Self {
            engine: engine.to_owned(),
            available,
            reason: reason.to_owned(),
        })
    }

    /// Имя движка на отчёте: по нему его и ищут в логах, а не по пути к программе.
    pub fn engine(&self) -> &str {
        &self.engine
    }

    /// Вердикт `probe()`.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Почему вердикт такой; непустая строка гарантирована построением.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Отказ движка. «Нет данных» и «отказ» обязаны быть разными вариантами, иначе
/// отсутствие причины читается как решение (закон 6).
#[derive(Debug, Error)]
pub enum EngineError {
    /// Причину не дали: пустая строка — это отсутствие данных, а не вердикт.
    #[error("probe report for `{engine}` was built without a reason")]
    NoReason { engine: String },
    /// Запускать нечего: подготовленный launch не назвал программу. Не «сессия не
    /// удалась»: её и не было.
    #[error("launch names no program: there is nothing to run")]
    EmptyArgv,
    /// Процесс не поднялся — движка не было. Это не отказ движка.
    #[error("`{program}` did not start: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    /// Бюджет исчерпан и процесс убит: «не успел» и «не запустился» чинятся по-разному.
    #[error("`{program}` exceeded its budget {timeout:?}")]
    TimedOut { program: String, timeout: Duration },
    /// Статус или вывод потеряны: сессия шла, а её слова не прочитаны.
    #[error("session output lost: {source}")]
    Io { source: std::io::Error },
}

/// Единый интерфейс запуска сессии: сказать, доступен ли движок, и провести одну сессию.
/// Бюджеты сверх `timeout`, песочница и одобрение — забота вызывающего (см. шапку модуля).
pub trait EngineAdapter {
    /// Доступность вместе с причиной, и ничего не запуская: список движков собирают, не
    /// поднимая их все, иначе нельзя ни показать отчёт, ни узнать, что Pi не установлен.
    fn probe(&self) -> Result<ProbeReport, EngineError>;

    /// Одна сессия: исполняет ПОДГОТОВЛЕННЫЙ запуск — команду, собранную вызывающим
    /// (`Sandbox::command` + явный набор переменных), а не собранную здесь из `argv`.
    /// Адаптер не собирает команду сам: иначе bwrap и `env_clear` остаются декорацией,
    /// а не гарантией (ADR-0016). Исчерпание бюджета — отдельный отказ.
    fn session(&self, launch: Command, timeout: Duration) -> Result<SessionOutcome, EngineError>;
}

/// Итог сессии. Ненулевой код — данные, а не отказ: агент сказал и ушёл.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOutcome {
    /// Код завершения; `None` — процесс убит сигналом, и кода у него нет.
    pub code: Option<i32>,
    /// stdout и stderr порознь: слияние теряет виновника.
    pub stdout: String,
    pub stderr: String,
}

/// Фикстура обязана называть себя фикстурой: движок, ведущий себя как настоящий, но
/// отчитавшийся настоящим, даёт ложный зелёный результат.
pub const FIXTURE_ENGINE: &str = "fixture-engine";

/// Фикстурный движок: контракт целиком, включая таймаут и недоступность, потому что
/// проверка границ доверия не должна зависеть от наличия настоящего Pi.
#[derive(Debug, Clone)]
pub struct FixtureEngine {
    program: PathBuf,
}

impl FixtureEngine {
    /// Конструктор НИЧЕГО не запускает, не пишет и не меняет окружения: собрав список
    /// движков, нельзя ни потребовать создать файл, ни поднять процесс — иначе список
    /// был бы «запустить всех, чтобы узнать, кто жив».
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }
}

impl EngineAdapter for FixtureEngine {
    fn probe(&self) -> Result<ProbeReport, EngineError> {
        let program = self.program.display();
        match executable(&self.program) {
            Ok(()) => {
                let why = format!("{FIXTURE_ENGINE}: `{program}` is an executable file");
                ProbeReport::available(FIXTURE_ENGINE, &why)
            }
            Err(why) => {
                let why = format!("{FIXTURE_ENGINE}: `{program}` is unusable: {why}");
                ProbeReport::unavailable(FIXTURE_ENGINE, &why)
            }
        }
    }

    fn session(
        &self,
        mut launch: Command,
        timeout: Duration,
    ) -> Result<SessionOutcome, EngineError> {
        // Запуск без программы — «запускать нечего», а не «сессия не удалась»: инциденты
        // разные. Программу называет подготовленный launch, а не адаптер.
        let program = launch.get_program().to_string_lossy().into_owned();
        if program.is_empty() {
            return Err(EngineError::EmptyArgv);
        }
        let mut child = launch
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| EngineError::Spawn {
                program: program.clone(),
                source,
            })?;
        // Бюджет держим опросом `try_wait`, а не `wait`: `wait` блокнулся бы навсегда.
        let deadline = Instant::now() + timeout;
        while !finished(&mut child)? && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        if !finished(&mut child)? {
            // Убиваем: иначе «бюджет исчерпан» означало бы «мы перестали смотреть».
            let _ = child.kill();
            return Err(EngineError::TimedOut {
                program: program.clone(),
                timeout,
            });
        }
        let Output {
            status,
            stdout,
            stderr,
        } = child
            .wait_with_output()
            .map_err(|source| EngineError::Io { source })?;
        Ok(SessionOutcome {
            code: status.code(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

/// Ещё жив ли процесс. «Не знаю» и «мертв» — не одно: ошибка чтения идёт своим отказом,
/// а не тихим `false`, который читался бы как «процесса нет».
fn finished(child: &mut Child) -> Result<bool, EngineError> {
    Ok(child
        .try_wait()
        .map_err(|source| EngineError::Io { source })?
        .is_some())
}

/// Доступность пути — это чтение МЕТАДАННЫХ, а не запуск: `probe()` не вправе поднимать
/// процесс. Право выполнения — часть доступности: неисполняемый файл запустить нельзя,
/// а каталог исполнить нельзя вовсе.
fn executable(program: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(program).map_err(|why| format!("metadata: {why}"))?;
    if !meta.is_file() {
        return Err(format!("not a file: `{}`", program.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!("not executable: `{}`", program.display()));
        }
    }
    Ok(())
}
