# packet-05
plan_id: M0
goal: Реализовать `module add` — валидация source/имени/дубликатов и append блока в modules.toml (RED→GREEN в одном packet).
files_allowed: [crates/stellwerk-registry/src/cmd_add.rs, crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry add
acceptance: RED-фаза: `bash -c 'cargo test -p stellwerk-registry add --no-run && ! cargo test -p stellwerk-registry add'` → 0; GREEN-фаза: `cargo test -p stellwerk-registry add` → 0; в tempdir modules.toml после успешного add появляется блок `[module.<name>]` с корректными ключами; валидные негативные кейсы (дубликат, bad name, неизвестный source-ключ, mcp без command, skill с command) → Err, файл не изменён.
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, #[ignore], #[allow], сеть/fetch, расширение CLI-флагов сверх спецификации]
depends_on: [packet-04]

## Комбинированный packet (RED→GREEN)

Lane m0/add: сначала падающие тесты (TDD), потом реализация — всё в одном packet и одной ветке. Evidence: сначала покажи RED-вывод, потом GREEN.

## Спецификация add

CLI уже разбирает AddArgs (packet-03/04). Реализуй `cmd_add::run(args, root: &Path) -> Result<(), AddError>`:

### Валидация (до записи)

1. Имя: regex `^[a-z0-9][a-z0-9_-]*$` (без regex-крейта — ручная проверка по байтам: первый [a-z0-9], остальные [a-z0-9_-]).
2. Ровно один source: git+rev ИЛИ npm ИЛИ cargo (не 0, не 2+). git требует непустой rev.
3. kind=mcp → `--command` обязателен (непустой); kind=skill → `--command` запрещён.
4. Дубликат: `let path = root.join("modules.toml"); let mf = ModulesFile::load(&path)?;` — имя уже есть в mf → Err. Никогда не передавай `root` (каталог) в `load` — только путь к файлу.
5. Никогда не вызывай сеть/git/fetch — только локальная валидация и запись.

### Запись (textual append, НЕ toml-roundtrip!)

Прочитай существующий modules.toml как строку, допиши в КОНЕЦ (с двойным переводом строки перед блоком), сохранив комментарии и порядок:

```toml

[module.<name>]
kind     = "mcp" | "skill"
autoload = true | false        # default true; --no-autoload → false
risk     = "<risk>"            # default "R3"; --risk переопределяет
source   = { git = "...", rev = "..." }  # или { npm = "...", version = "..." } или { cargo = "...", version = "..." }
command  = "..."               # только для kind=mcp
tools    = ["a", "b"]          # из --tools (split по ',' ); всегда писать ключ: пустой/нет флага → tools = []
context_cost_tokens = 0        # всегда 0 до measure
update   = { channel = "<ch>", policy = "pr" }  # только если задан --update-channel; иначе строку update не добавлять
```

Формат записи: один пробел вокруг `=` у всех простых ключей нового блока (`key = value`), inline-таблицы `key = { k = v, ... }` в одну строку, массивы `["a", "b"]` без висячей запятой. Не обязан повторять дырявое выравнивание module.polyforge — единственный жёсткий критерий: после записи `ModulesFile::load(&root.join("modules.toml"))` успешно парсит файл (round-trip check в тесте).

### Wiring (lib.rs + cli.rs)

- lib.rs: append `pub mod cmd_add;` одной строкой ПОСЛЕ последней существующей `pub mod` (не в середину, не удаляй чужие) — при merge параллельных lanes оставляй все `pub mod` (см. merge-протокол в M0-index).
- cli.rs dispatch: append match-arm `ModuleCommand::Add(args) → cmd_add::run(&args, root)` в КОНЕЦ существующего match по ModuleCommand (чужие arms не трогай). Ok → 0, Err → eprintln + 1.

root для тестов = tempdir; для бинаря = cwd. Вынеси `run(args, root: &Path)` чтобы тесты передавали tempdir без переменной окружения.

## Тесты (filter `add`, в cmd_add.rs `#[cfg(test)]`; tempdir через std::env::temp_dir() + unique, без tempfile)

RED (напиши первыми, до реализации — в RED-фазе модуль может быть пустой заглушкой `unimplemented!()` чтобы --no-run прошёл, а тесты падали):

1. valid git add → файл содержит `[module.foo]`, git, rev, kind=mcp, command, autoload=true, risk=R3, context_cost_tokens=0; load() парсит; имя foo есть в map.
2. valid npm add (с и без --npm-version) → source npm ключи.
3. valid cargo add → source cargo ключи.
4. skill без command → OK; skill с command → Err; mcp без command → Err.
5. дубликат (дважды add одного имени) → второй Err, файл не изменён (сравни строки до/после).
6. bad имя (`Foo`, `-x`, `a b`, пустое) → Err.
7. 0 или 2 source-флага → Err.
8. --no-autoload → autoload=false; без него → true.
9. --update-channel tag → строка `update = { channel = "tag", policy = "pr" }` (один пробел вокруг `=`); без флага → в блоке нет `update`; `--tools` пустой/нет → ключ `tools = []` присутствует.
10. существующий modules.toml с комментарием и module.polyforge — после add комментарий и polyforge на месте, append в конце.
11. context_cost_tokens всегда 0 (не трогай modules.lock).

GREEN: те же тесты зелёные без ослабления.

## Evidence

- RED: `bash -c 'cargo test -p stellwerk-registry add --no-run && ! cargo test -p stellwerk-registry add'` → 0 + вывод.
- GREEN: `cargo test -p stellwerk-registry add` → 0 + вывод.
- PROOF: evidence_append + evidence_verify(cmd='cargo test -p stellwerk-registry add').
- GATE: gate_evaluate.
- STATUS/FILES/TEST/EVIDENCE.
