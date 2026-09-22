# packet-01
plan_id: M0
goal: Создать модельные типы registry (Source/Module/Calibration/ModulesFile) и формулу tokens=round(bytes×tokens_per_byte) — падающие тесты + компилируемые заглушки.
files_allowed: [crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/model.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry model
acceptance: RED: bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry model --no-run && ! cargo test -p stellwerk-registry model' → exit 0 (build OK, тесты компилируются, прогон падает на assert, НЕ ошибка компиляции).
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, #[ignore], #[allow], исправление/ослабление теста]
depends_on: []

## RED-пакет — правила

DO NOT fix/red-green the test inside a RED packet. Напиши падающий тест и компилируемую заглушку с заведомо неверным поведением. Зелёным сделает packet-02 (lane-model, та же ветка). Доказательство = вывод RED-wrapper выше (должен вернуть 0).

## Контекст

- Crate: `stellwerk-registry`, bin `stellwerk` (src/main.rs не трогать). Ты создаёшь lib-таргет: `src/lib.rs` автообнаруживается Cargo, правка Cargo.toml НЕ нужна и запрещена.
- Типы живут в `src/model.rs` (решение Q1: contract.rs заморожен на весь M0).
- Формула (Q2): `tokens = round(bytes × tokens_per_byte)`. В modules.toml сейчас ключ `bytes_per_token` — принимай его через `#[serde(alias = "bytes_per_token")]` на поле `tokens_per_byte`; packet-07b переименует ключ в файле, код не меняется.
- Source (Q4), untagged enum, TOML-ключи: `git`+`rev` | `npm`[+`version`] | `cargo`[+`version`]; неизвестные ключи отклонять.

## Что сделать

1. `src/lib.rs` (новый):
```rust
pub mod model;
```
2. `src/model.rs` — типы с serde-производными (нужны, чтобы тесты компилировались) + заглушки, возвращающие ошибку/0:
   - `ModelError` (thiserror): `NotImplemented`, `Toml(#[from] toml::de::Error)`, `Io(#[from] std::io::Error)`.
   - `Calibration { tokens_per_byte: f64 (alias "bytes_per_token"), measured_at: String }`.
   - `ModuleKind` = `Mcp | Skill` (serde lowercase).
   - `Source` untagged: `Git { git, rev }`, `Npm { npm, version? }`, `Crate { #[serde(rename="cargo")] krate, version? }`; variants с `deny_unknown_fields` (отклонение мусора).
   - `UpdatePolicy { channel: String, policy: String }`.
   - `Module` (`deny_unknown_fields`): kind, autoload (default true), risk: Option<String>, source, command: Option<String>, tools: Vec<String> (default), context_cost_tokens: u64 (default), update: Option<UpdatePolicy>.
   - `Registry` { budgets Option<u64>, calibration: Calibration } + `ModulesFile { registry, module: BTreeMap<String, Module> }`.
   - `ModulesFile::from_toml_str(&str) -> Result<Self, ModelError>` — RED: всегда `Err(NotImplemented)`.
   - `ModulesFile::load(&Path) -> Result<Self, ModelError>` — RED: всегда `Err(NotImplemented)`.
   - `tokens_for(bytes: u64, tokens_per_byte: f64) -> u64` — RED: всегда `0` (игнорирует вход).
3. Никакой сети, никакого CLI, никакого main.rs.

## Тесты (RED — напиши ПЕРВЫМИ, в model.rs `#[cfg(test)] mod tests`; путь теста содержит `model`)

- FIXTURE: строка TOML с `[registry.calibration] bytes_per_token = 0.5` + `[module.polyforge]` (kind=mcp, source git+rev, command, tools, update={channel=tag,policy=pr}).
- `from_toml_str(FIXTURE)` → Ok: kind==Mcp, Source::Git с верными url/rev, calibration.tokens_per_byte==0.5 (alias), budgets на месте.
- `from_toml_str` с мусорным ключом в `[module.x]` → Err (deny_unknown_fields).
- `from_toml_str` с мусором в source (`{ foo = "x" }`) → Err.
- `load(tempdir/modules.toml)` → Ok (создай tempdir через `std::env::temp_dir()` + unique suffix, без крейта tempfile — новых зависимостей нет).
- `tokens_for(1000, 0.813) == 813`; `tokens_for(0, 0.813) == 0`; `tokens_for(1, 0.5) == 1` (round half away from zero: 0.5→1).
- Source serde round-trip: Git/Npm/Crate variant → to_string → from_str равны.

Ожидание RED: часть тестов (формула, load, from_toml_str) падает на assert/unwrap из-за NotImplemented/0; --no-run проходит.

## Evidence

- RED: `bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry model --no-run && ! cargo test -p stellwerk-registry model'` → 0; вставь вывод падающего теста.
- PROOF (после GREEN в packet-02, не здесь): evidence_append + evidence_verify(cmd='cargo test -p stellwerk-registry model').
- Формат ответа: STATUS/FILES/TEST/EVIDENCE (AGENTS.md).
