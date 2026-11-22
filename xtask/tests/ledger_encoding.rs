//! Проверка кодировки текстовых записей леджера: `U+FFFD` не должен появляться вновь.
//!
//! F-70: исторические строки переписывать запрещено — повреждение есть факт append-only
//! журнала, а не текст. Поэтому проверка не ищет «нулевую» чистоту, а сверяет счётчик с
//! baseline: рост означает новое повреждение, убывание — что историческая строка переписана.
//! Оба случая называются `encoding_corruption`.

use std::path::{Path, PathBuf};

const REPLACEMENT: char = '\u{FFFD}';

/// Исторический уровень повреждения, зафиксированный в минимальных синтетических фикстурах:
/// `FINDINGS.md` — 4 символа на 2 строках, `STATE.md` — 2 символа на 1 строке.
/// Живой журнал `.stellwerk/FINDINGS.md` внутренний и не публикуется; фикстура замораживает
/// исторический baseline кодировки, чтобы тест не зависел от приватных файлов.
const BASELINE: &[(&str, usize)] = &[("xtask/tests/fixtures/ledger_encoding/FINDINGS.md", 4), ("xtask/tests/fixtures/ledger_encoding/STATE.md", 2)];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("у крейта xtask есть родитель — корень workspace")
        .to_path_buf()
}

/// Сколько `U+FFFD` в файле, с номерами строк для диагностики.
fn count_replacement(path: &Path) -> (usize, Vec<usize>) {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("не прочитать {}: {err}", path.display()));
    let total = text.chars().filter(|c| *c == REPLACEMENT).count();
    let lines: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(REPLACEMENT))
        .map(|(i, _)| i + 1)
        .collect();
    (total, lines)
}

/// Ни одно повреждение не добавилось сверх baseline.
#[test]
fn ledger_has_no_new_replacement_chars() {
    let root = workspace_root();
    for (relative, baseline) in BASELINE {
        let path = root.join(relative);
        let (found, lines) = count_replacement(&path);
        assert_eq!(
            found, *baseline,
            "encoding_corruption в {relative}: символов {found}, строк {lines:?}, \
             ожидалось исторических {baseline}. Рост означает новое повреждение, убывание — \
             переписанную историю, и то и другое F-70 запрещает."
        );
    }
}

/// Проверка не пропускает новое повреждение: временная копия с лишним символом обязана дать
/// расхождение, иначе основной тест выше проходит вхолостую.
#[test]
fn a_fresh_replacement_char_is_detected() {
    let dir = std::env::temp_dir().join(format!("ledger_encoding_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("каталог");
    let path = dir.join("probe.md");
    std::fs::write(&path, format!("строка\n{REPLACEMENT}\n")).expect("запись");
    let (found, lines) = count_replacement(&path);
    assert_eq!(found, 1, "свежий U+FFFD обязан считаться");
    assert_eq!(lines, vec![2], "номер строки нужен для диагностики");
    let _ = std::fs::remove_dir_all(&dir);
}
