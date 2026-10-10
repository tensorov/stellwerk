//! Тулчейн мутационного пути: nightly задаётся явно и только на мутации.
//!
//! `rust-toolchain.toml` пинит 1.99.0 намеренно (коммит `e35090e`), и этот пин
//! остаётся. Но `cargo mutants` требует nightly, поэтому мутационный путь обязан
//! нести `RUSTUP_TOOLCHAIN` сам, а не наследовать stable-пин из соседнего файла
//! (F-91). Сборка окружения вынесена в функцию, и тест зовёт её напрямую — сам
//! `cargo mutants` здесь не запускается.

#[path = "../src/main.rs"]
pub mod xtask;

use std::process::Command;

/// Мутационный путь несёт явный nightly. Если оверрайд убрать, дочерний `cargo`
/// снова унаследует 1.99.0 из `rust-toolchain.toml`, и гейт уйдёт на stable.
#[test]
fn mutation_path_pins_nightly_explicitly() {
    let env = xtask::mutation_env();
    assert!(
        env.contains(&("RUSTUP_TOOLCHAIN", "nightly-2026-10-01")),
        "мутационный путь не задаёт RUSTUP_TOOLCHAIN явно: {env:?}"
    );
}

/// Общий путь гейта не получает RUSTUP_TOOLCHAIN: ночной остаётся только мутация.
#[test]
fn gate_path_does_not_pin_nightly() {
    let env = xtask::gate_env();
    assert!(
        !env.iter().any(|(key, _)| *key == "RUSTUP_TOOLCHAIN"),
        "общий путь гейта получил RUSTUP_TOOLCHAIN: {env:?}"
    );
}

/// Оверрайд указывает на установленный тулчейн, а не на выдуманное имя: иначе
/// явный пин был бы неотличим от опечатки.
#[test]
fn nightly_toolchain_is_installed() {
    let out = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .expect("rustup должен быть в PATH");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("nightly-2026-10-01"),
        "nightly-2026-10-01 не установлен: {stdout}"
    );
}
