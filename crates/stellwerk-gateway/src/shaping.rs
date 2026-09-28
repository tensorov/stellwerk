//! Усечение вывода: длинный ответ не льётся в контекст, а превращается в хеш
//! и путь к артефакту. 2k токенов на вывод — самый дешёвый по токенам и самый
//! дорогой по деньгам механизм проекта: воркер живёт в окне ~43k. Наружу уходят
//! хеш и путь; тело вывода наружу не уходит.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Порог вывода в токенах, выше которого тело заменяется хешем и путём.
/// Переопределяется `STELLWERK_MAX_OUTPUT_TOKENS`.
pub const MAX_OUTPUT_TOKENS: u64 = 2048;

/// Переменная окружения с порогом.
const ENV_THRESHOLD: &str = "STELLWERK_MAX_OUTPUT_TOKENS";

/// Что возвращается в контекст вместо тела вывода.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shaped {
    /// То, что уходит в контекст: при усечении — начало вывода с маркером обреза.
    pub text: String,
    /// Тело не поместилось в порог и ушло в артефакт.
    pub truncated: bool,
    /// SHA-256 **полного** вывода, hex в нижнем регистре. По усечённому тексту
    /// считать нельзя: ссылка на артефакт перестала бы соответствовать ему.
    /// Заполнено всегда: хеш определён и для неусечённого вывода, а пустая
    /// строка означала бы «не посчитали» — состояния, у хеша не бывает.
    pub sha256: String,
    /// Путь к артефакту с полным телом. `None` при `truncated = false`:
    /// путь на несуществующий файл — тихий сбой, который вскроется через месяц.
    pub artifact_path: Option<PathBuf>,
}

/// Что может помешать усечению.
///
/// `Config` отделён от `Io` намеренно: нечитаемый порог — опечатка в конфиге, а
/// не сбой диска, и молчаливый откат на 2048 тихо отключил бы защиту.
#[derive(Debug, Error)]
pub enum ShapeError {
    /// Каталог артефактов недоступен.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Порог из окружения или калибровка токенов нечитаемы.
    #[error("config: {0}")]
    Config(String),
}

/// Усечь вывод либо, если он помещается в порог, вернуть как есть.
///
/// `tokens_per_byte` приходит аргументом: калибровка измерена и лежит в
/// `modules.lock`, хардкод здесь означал бы, что замер стоимости байта выкинут.
pub fn shape(output: &str, tokens_per_byte: f64, dir: &Path) -> Result<Shaped, ShapeError> {
    // Калибровка не «нулевая цена байта», а отсутствие калибровки: при ней
    // бюджет вышел бы больше самого вывода и обрезать было бы нечего.
    if !tokens_per_byte.is_finite() || tokens_per_byte <= 0.0 {
        return Err(ShapeError::Config(format!(
            "tokens_per_byte должен быть конечным и положительным, получено {tokens_per_byte}"
        )));
    }
    let threshold = threshold()?;
    let sha256 = sha256_hex(output.as_bytes());
    let tokens = (output.len() as f64 * tokens_per_byte).round() as u64;

    if tokens <= threshold {
        // Тело в порог поместилось: ни каталога, ни файла не создаём. `None`
        // вместо пути — путь на несуществующий файл не отличить от «нет файла».
        return Ok(Shaped {
            text: output.to_owned(),
            truncated: false,
            sha256,
            artifact_path: None,
        });
    }

    let artifact_path = write_artifact(dir, &sha256, output)?;
    let budget = (threshold as f64 / tokens_per_byte) as usize;
    let head = char_boundary_prefix(output, budget);
    let text = format!(
        "{head}\n\n[stellwerk: обрезано, {tokens} токенов > {threshold}; полное тело: \
         sha256 {sha256} @ {path}]",
        path = artifact_path.display(),
    );
    Ok(Shaped {
        text,
        truncated: true,
        sha256,
        artifact_path: Some(artifact_path),
    })
}

/// Порог из окружения. Отсутствие переменной — документированный дефолт;
/// нечитаемое значение — ошибка, а не откат.
fn threshold() -> Result<u64, ShapeError> {
    match std::env::var(ENV_THRESHOLD) {
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .map_err(|err| ShapeError::Config(format!("{ENV_THRESHOLD}={raw:?}: {err}"))),
        Err(std::env::VarError::NotPresent) => Ok(MAX_OUTPUT_TOKENS),
        Err(err) => Err(ShapeError::Config(format!("{ENV_THRESHOLD}: {err}"))),
    }
}

/// Префикс длиной не более `budget` байт, обрезанный по границе символа: резать
/// посреди многобайтного UTF-8 нельзя, в контекст уйдёт битый вывод.
fn char_boundary_prefix(text: &str, budget: usize) -> &str {
    let mut cut = budget.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

/// Полное тело под именем хеша: тот же вывод перезапишет себя, а не заплодит
/// каталог дублями. Пишем во временный файл и переименовываем — читатель
/// никогда не увидит наполовину записанный артефакт.
fn write_artifact(dir: &Path, sha256: &str, body: &str) -> Result<PathBuf, ShapeError> {
    let name = format!("{sha256}.txt");
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{name}.tmp"));
    fs::write(&temp, body)?;
    let path = dir.join(name);
    fs::rename(&temp, &path)?;
    Ok(path)
}

/// Hex в нижнем регистре — ровно то, чем назван артефакт, чтобы сверка шла глазами.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
