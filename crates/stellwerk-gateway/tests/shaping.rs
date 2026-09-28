//! Tests for `shape` — the output-shaping surface (M3/packet-05 RED).
//! RED: `shape` — заглушка, все шесть сценариев падают, прогон компилируется.
//! Модуль включён через `#[path]`: `files_allowed` не даёт трогать `src/lib.rs`,
//! а `pub mod shaping;` там объявит отдельный пакет.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use sha2::{Digest, Sha256};

#[path = "../src/shaping.rs"]
mod shaping;

use shaping::{shape, ShapeError, MAX_OUTPUT_TOKENS};

/// Порог из окружения — процесс-глобальный: опущенный не должен быть виден
/// никому, кроме теста, который его опустил.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Калибровка приходит аргументом, из `modules.lock`, а не хардкодом.
const TOKENS_PER_BYTE: f64 = 0.25;
const ENV_THRESHOLD: &str = "STELLWERK_MAX_OUTPUT_TOKENS";

fn lock_env() -> MutexGuard<'static, ()> {
    // Отравленный мьютекс после паники соседнего теста — не повод падать и здесь.
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Подкаталог `temp_dir` на время теста; `create_dir_all`, а не `create_dir`:
/// после упавшего прогона каталог остаётся, и это не повод падать.
struct ArtifactDir(PathBuf);

impl ArtifactDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("stellwerk-shaping-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).expect("каталог артефактов создаётся");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ArtifactDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Подмена порога с восстановлением: упавший тест не должен оставлять процесс
/// в изменённом состоянии.
struct EnvOverride(Option<String>);

impl EnvOverride {
    fn set(value: &str) -> Self {
        let previous = std::env::var(ENV_THRESHOLD).ok();
        std::env::set_var(ENV_THRESHOLD, value);
        Self(previous)
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        match &self.0 {
            Some(previous) => std::env::set_var(ENV_THRESHOLD, previous),
            None => std::env::remove_var(ENV_THRESHOLD),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn short_output_passes_through_byte_for_byte_without_artifact() {
    let _env = lock_env();
    let dir = ArtifactDir::new("short");
    let output = "x".repeat(1_000); // 250 токенов при 0.25
    let shaped = shape(&output, TOKENS_PER_BYTE, dir.path()).expect("короткий вывод не падает");
    assert!(!shaped.truncated, "250 токенов < {MAX_OUTPUT_TOKENS}");
    assert_eq!(shaped.text, output, "байт в байт");
    // Путь на несуществующий артефакт — тихий сбой: его не отличить от «нет артефакта».
    assert!(shaped.artifact_path.is_none(), "пути быть не должно");
}

#[test]
fn long_output_becomes_hash_and_path_instead_of_a_flood() {
    let _env = lock_env();
    let dir = ArtifactDir::new("long");
    let output = "x".repeat(100_000); // 25_000 токенов при 0.25
    let shaped = shape(&output, TOKENS_PER_BYTE, dir.path()).expect("длинный вывод не падает");
    assert!(shaped.truncated, "25_000 токенов > {MAX_OUTPUT_TOKENS}");
    assert!(!shaped.sha256.is_empty(), "усечение без хеша");
    let path = shaped.artifact_path.expect("путь к артефакту");
    assert!(path.is_file(), "артефакт должен существовать");
    let name = format!("{}.txt", shaped.sha256);
    assert_eq!(path, dir.path().join(name), "имя от хеша");
    assert!(shaped.text.len() < output.len(), "в контекст ушло меньше");
}

#[test]
fn artifact_body_equals_the_full_original_output() {
    let _env = lock_env();
    let dir = ArtifactDir::new("body");
    let output = "x".repeat(100_000);
    let shaped = shape(&output, TOKENS_PER_BYTE, dir.path()).expect("длинный вывод не падает");
    let path = shaped.artifact_path.expect("путь к артефакту");
    assert_eq!(fs::read_to_string(&path).expect("читается"), output);
}

#[test]
fn sha256_is_over_the_full_body_not_the_truncated_text() {
    // Отдельный тест, а не следствие проверки артефакта: хеш по усечённому тексту
    // дал бы ссылку на артефакт, который этому хешу не соответствует, и сверка
    // разошлась бы молча, уже не на тестах.
    let _env = lock_env();
    let dir = ArtifactDir::new("hash");
    let output = "x".repeat(100_000);
    let shaped = shape(&output, TOKENS_PER_BYTE, dir.path()).expect("длинный вывод не падает");
    let full = sha256_hex(output.as_bytes());
    let cut = sha256_hex(shaped.text.as_bytes());
    assert_eq!(shaped.sha256, full, "хеш по полному телу");
    assert_ne!(shaped.sha256, cut, "не по усечённому");
}

#[test]
fn threshold_from_environment_is_respected() {
    let _env = lock_env();
    let dir = ArtifactDir::new("env");
    let output = "x".repeat(200); // 50 токенов при 0.25
    let _threshold = EnvOverride::set("8");
    let shaped = shape(&output, TOKENS_PER_BYTE, dir.path()).expect("вывод не падает");
    assert!(shaped.truncated, "порог 8 ниже 50 токенов");
    assert!(!shaped.sha256.is_empty(), "хеш усечения");
}

#[test]
fn unusable_artifact_dir_is_an_io_error_not_a_panic() {
    let _env = lock_env();
    // `dir` занят обычным файлом: каталог не существует и не может быть создан —
    // это `Io`, а не паника и не тихое «сохранил в никуда».
    let dir = ArtifactDir::new("io");
    let occupied = dir.path().join("occupied");
    fs::write(&occupied, "не каталог").expect("занять путь файлом");
    let result = shape(&"x".repeat(100_000), TOKENS_PER_BYTE, &occupied);
    assert!(matches!(result, Err(ShapeError::Io(_))), "{result:?}");
}
