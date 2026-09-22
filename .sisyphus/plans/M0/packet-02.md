# packet-02
plan_id: M0
goal: Реализовать model.rs — from_toml_str/load/tokens_for зелёные, тесты packet-01 проходят без ослабления assert'ов.
files_allowed: [crates/stellwerk-registry/src/model.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry model
acceptance: cargo test -p stellwerk-registry model → exit 0; cargo build -p stellwerk-registry → exit 0; ни один тест из packet-01 не удалён/не ослаблен.
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, правка lib.rs, #[ignore], #[allow], удаление/ослабление тестов]
depends_on: [packet-01]

## GREEN-пакет

Компилируемые заглушки packet-01 заменяются минимальной рабочей реализацией. TDD: сначала посмотри падающие тесты, потом код. Никакого «рефакторинга заодно».

## Что сделать (только model.rs)

1. `ModulesFile::from_toml_str(s)` → `Ok(toml::from_str(s)?)` (serde-производные уже на типах).
2. `ModulesFile::load(path)` → `std::fs::read_to_string(path)?` + `from_toml_str`.
3. `tokens_for(bytes, tokens_per_byte)` → `(bytes as f64 * tokens_per_byte).round() as u64`.
4. Не трогай типы/атрибуты (alias, deny_unknown_fields) — если тест на них падал, значит атрибут был не там; поправь минимально, не ослабляя `deny_unknown_fields` и alias.

## Приёмка

- `cargo test -p stellwerk-registry model` → все зелёные.
- `cargo build -p stellwerk-registry` → 0.
- `cargo clippy -p stellwerk-registry --all-targets -- -D warnings` → без warning'ов (workspace lints: clippy::all deny).

## Evidence

- GREEN: вставь вывод проходящего `cargo test -p stellwerk-registry model`.
- PROOF: evidence_append(ModelClaim) + evidence_verify(cmd='cargo test -p stellwerk-registry model').
- GATE: gate_evaluate; fail → не отчитывайся об успехе.
- Формат ответа: STATUS/FILES/TEST/EVIDENCE.
