# packet-03
plan_id: M0
goal: CLI-скелет clap (stellwerk module add|measure|audit|update) с parse_from — падающие тесты разбора + заглушка Err.
files_allowed: [crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry cli
acceptance: RED: bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry cli --no-run && ! cargo test -p stellwerk-registry cli' → exit 0 (компилируется, падает на assert).
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, правка main.rs, правка model.rs, #[ignore], #[allow], исправление теста]
depends_on: [packet-02]

## RED-пакет — правила

DO NOT fix/red-green the test inside a RED packet. Зелёным сделает packet-04 (lane-cli). Только падающие тесты разбора + компилируемая заглушка.

## Контекст

- bin `stellwerk`, команды: `module add <NAME> --kind <mcp|skill> (--git <URL> --rev <REV> | --npm <PKG> [--npm-version <VER>] | --cargo <CRATE> [--cargo-version <VER>]) [--command <BIN>] [--tools t1,t2] [--risk <RISK>] [--no-autoload] [--update-channel <CHANNEL>]`, `module measure [<NAME>]`, `module audit [<NAME>]`, `module update [<NAME>]`.
- Exit-коды (для packet-04): 0 ok, 1 runtime error, 2 clap/usage. В этом packet только `parse_from`, без `run`.
- clap 4.x derive, уже в workspace deps. Новых зависимостей нет.
- lib.rs сейчас содержит `pub mod model;` — добавь строку `pub mod cli;` (не удаляй model).

## Что сделать

1. `src/cli.rs`:
   - `Cli` (Parser): name=stellwerk, about; subcommand `Command::Module { command: ModuleCommand }`.
   - `ModuleCommand` (Subcommand): `Add(AddArgs)`, `Measure(NameArgs)`, `Audit(NameArgs)`, `Update(NameArgs)`.
   - `AddArgs` (Args): `name: String`; `--kind` → `KindArg` (ValueEnum: Mcp, Skill; принимается ТОЛЬКО нижний регистр `mcp`/`skill` — `--kind MCP` → clap usage error exit 2, не нормализуйся); source-флаги `--git/--rev/--npm/--npm-version/--cargo/--cargo-version` как Option (валидация комбинаций — packet-05); `--command: Option<String>`; `--tools` `value_delimiter=','` → Vec<String>; `--risk: Option<String>`; `--no-autoload` flag; `--update-channel: Option<String>`.
   - `NameArgs`: `name: Option<String>`.
   - `pub fn parse_from<I, T>(args: I) -> Result<Cli, clap::Error>` — RED: всегда `Err(clap::Error::raw(ErrorKind::InvalidValue, "RED: not implemented"))`.
2. `src/lib.rs`: добавить `pub mod cli;`.
3. НЕ трогай main.rs (dispatch — packet-04).

## Тесты (RED — в cli.rs `#[cfg(test)] mod tests`; путь содержит `cli`)

Только валидные входы, ожидающие Ok (иначе заглушка-Err даст ложный PASS на негативных кейсах):

- `parse_from(["stellwerk","module","add","polyforge","--kind","mcp","--git","https://example.com/x","--rev","abc123","--command","polyforge-mcp","--tools","a,b","--risk","R3","--update-channel","tag"])` → Ok; сверяй поля (name, kind==Mcp, git, rev, command, tools==["a","b"], risk, update_channel, !no_autoload).
- `parse_from([...,"--kind","skill","--npm","pkg","--npm-version","1.0"])` → Ok; kind==Skill, npm/pkg, npm_version.
- `parse_from([...,"--kind","mcp","--cargo","some-crate","--cargo-version","2.0"])` → Ok.
- `parse_from([...,"--no-autoload"])` → Ok; no_autoload==true.
- `parse_from(["stellwerk","module","measure","polyforge"])` → Ok; name==Some("polyforge").
- `parse_from(["stellwerk","module","measure"])` → Ok; name==None.
- `parse_from(["stellwerk","module","audit","x"])`, `parse_from(["stellwerk","module","update"])` → Ok (диспетчерные типы).

`assert!`/`match` на полях; unwrap_err недопустим для позитивных кейсов. Ожидание: все падают, т.к. parse_from всегда Err.

## Evidence

- RED: `bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry cli --no-run && ! cargo test -p stellwerk-registry cli'` → 0; вывод падающих тестов.
- Формат ответа: STATUS/FILES/TEST/EVIDENCE.
