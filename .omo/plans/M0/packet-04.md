# packet-04
plan_id: M0
goal: Реализовать parse_from через clap + run() с диспетчеризацией на placeholder-и команд (exit 1) и подключить main.rs (exit 0/1/2).
files_allowed: [crates/stellwerk-registry/src/cli.rs, crates/stellwerk-registry/src/main.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry cli
acceptance: cargo test -p stellwerk-registry cli → exit 0 (старые тесты packet-03 + новые для run/help/usage); cargo build -p stellwerk-registry → 0; ручной smoke: `target/debug/stellwerk --help` → 0, `target/debug/stellwerk bad` → 2, `target/debug/stellwerk module add x --kind mcp` (валидно clap'ом) → 1 (placeholder).
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, правка lib.rs, правка model.rs, реализация add/measure/audit/update (это packets 05+), #[ignore], #[allow], ослабление тестов packet-03]
depends_on: [packet-03]

## GREEN-пакет

Сделай зелёными тесты packet-03 (parse_from) и добавь покрытие run/exit-кодов. Команды остаются placeholder'ами (return 1) — их реализуют packets 05/07a/09/11.

## Что сделать (cli.rs)

1. `parse_from`:
```rust
pub fn parse_from<I, T>(args: I) -> Result<Cli, clap::Error>
where I: IntoIterator<Item = T>, T: Into<std::ffi::OsString> + Clone
{
    Cli::try_parse_from(args)
}
```
2. `pub fn run(args: Vec<std::ffi::OsString>) -> i32`:
   - `parse_from(&args)` → Err: если kind ∈ {DisplayHelp, DisplayVersion} — распечатай (`e.print()`) и верни 0; иначе `e.print()` и верни 2.
   - Ok → `dispatch(cli)`:
     - `ModuleCommand::Add(_)` → eprintln «module add: not implemented (packet-05)» → 1.
     - `Measure(_)` → eprintln «… (packet-07a)» → 1.
     - `Audit(_)` → eprintln «… (packet-09)» → 1.
     - `Update(_)` → eprintln «… (packet-11)» → 1.
3. `src/main.rs` (replace 5-строчный заглушка, ≤15 строк):
```rust
fn main() {
    let code = stellwerk_registry::cli::run(std::env::args_os().collect());
    std::process::exit(code);
}
```
Имя lib-крейта: `stellwerk_registry` (дефис → подчёркивание).

## Тесты (добавить в cli.rs tests; существующие НЕ ослаблять)

- Тесты packet-03 (parse_from → Ok + поля) — теперь зелёные.
- `run(vec!["stellwerk".into(), "badcmd".into()]) == 2`.
- `run(vec!["stellwerk".into(), "module".into()]) == 2` (нет подкоманды).
- `run(vec!["stellwerk".into(), "--help".into()]) == 0`.
- `run(vec!["stellwerk".into(), "module".into(), "add".into(), "polyforge".into(), "--kind".into(), "mcp".into(), "--git".into(), "https://e/x".into(), "--rev".into(), "abc".into()]) == 1` (placeholder, валидный clap).

Smoke вручную после сборки (не как cargo test): `--help`→0, `bad`→2, валидный add без dispatch-логики→1.

## Приёмка

- `cargo test -p stellwerk-registry cli` → 0.
- `cargo clippy -p stellwerk-registry --all-targets -- -D warnings` → 0.
- После merge packet-04 в main открывается Wave 3 (add|measure|audit параллельно).

## Evidence

- GREEN: вывод `cargo test -p stellwerk-registry cli`.
- PROOF: evidence_append + evidence_verify(cmd='cargo test -p stellwerk-registry cli').
- GATE: gate_evaluate.
- Формат ответа: STATUS/FILES/TEST/EVIDENCE.
