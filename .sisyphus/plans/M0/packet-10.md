# packet-10
plan_id: M0
goal: update RED — CommandRunner trait (git/cargo/gh через шов), STELLWERK_GH_BIN, логика no-gh → .stellwerk/pending/ патч, заглушки падающих тестов.
files_allowed: [crates/stellwerk-registry/src/lib.rs, crates/stellwerk-registry/src/cmd_update.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry update
acceptance: RED: bash -c 'cargo build -p stellwerk-registry && cargo test -p stellwerk-registry update --no-run && ! cargo test -p stellwerk-registry update' → exit 0.
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock, правка contract.rs, правка cli.rs/main.rs, реальный git/cargo/gh в тестах, реальная сеть, merge PR, #[ignore], #[allow], исправление теста]
depends_on: [packet-07b, packet-09]

## RED-пакет — правила

DO NOT fix/red-green the test inside a RED packet. Зелёным сделает packet-11 (lane-update, последняя волна). Никаких реальных внешних вызовов в тестах — только фейковый runner.

## Контекст (Q5 + поправка D)

update flow: fetch source (git ls-remote/checkout rev во временной каталог, модули снаружи) → self `cargo build -p stellwerk-registry` → audit (injected) → measure (injected) → diff modules.toml/lock → `gh pr create` (base = default branch, title/body, **НИКОГДА merge**). 

Поправка D (verbatim в комментариях тестов/кода):

> update reads binary from env STELLWERK_GH_BIN (default `gh`); tests inject fixture script; missing-binary scenario → .stellwerk/pending/ + stop covered by test with nonexistent bin; real gh only in manual final M0 acceptance. Pending dir is `.stellwerk/pending/` (NOT legacy `.nexus/pending/`).

- rev bump: channel=tag → в блоке modules.toml модуля поле rev заменяется (string surgery внутри `[module.<name>]` до следующего `[` или EOF).
- PR: только modules.lock (+modules.toml rev bump при channel=tag).
- pending: если STELLWERK_GH_BIN не существует/не запускается → записать патч `.stellwerk/pending/<name>-<epoch>.patch` формата:
  `### <file> (before)\n<содержимое>\n### <file> (after)\n<содержимое>\n` для каждого изменённого файла; stdout `pending: <path>`; **exit 0** (стоп без ошибки).
- Exit: 0 = ok/no-update/pending; 1 = ошибка fetch/build/audit/measure.

## Что сделать

1. lib.rs: `pub mod cmd_update;`
2. cmd_update.rs — шов + заглушки RED:
   - `pub trait CommandRunner { fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<CmdOutput, UpdateError>; }` CmdOutput { code: i32, stdout: String, stderr: String }.
   - `pub struct RealRunner;` impl — `std::process::Command`; spawn-ошибка (бинарь не найден и т.п.) → `Err(UpdateError::Spawn)`, без паники (используется только prod в 11; в 08/10 тесты всегда фейк).
   - `pub enum GhMode { Binary(PathBuf), Missing }` + `fn gh_binary() -> GhMode` читает env `STELLWERK_GH_BIN` default `"gh"`, проверяет existence.
   - `pub fn resolve_new_rev(runner, url, current_rev, channel) -> Result<Option<String>, UpdateError>` — ls-remote --tags; RED: Err/Ok(None) ломающим тест.
   - `pub fn bump_rev(modules_toml: &str, name: &str, new_rev: &str) -> String` — string surgery; RED: возвращает вход без изменений.
   - `pub fn write_pending_patch(root: &Path, name: &str, epoch: u64, diffs: &[(String,String,String)]) -> Result<PathBuf, UpdateError>` — before/after секции; RED: Err.
   - `pub fn update_module(...)` оркестратор с инжектированными `audit: Fn` и `measure: Fn` (Fn(Path, name) -> Result<..>) — чтобы тесты не звали реальные cmd_audit/cmd_measure; RED: Err(NotImplemented).
3. Никакого cli.rs в этом packet.

## Тесты (filter `update`, в cmd_update.rs tests; фейковый runner, инжектированные audit/measure, фейковые «ссылки» на git)

1. GhMode: env STELLWERK_GH_BIN=/nonexistent/bin → Missing; запись env в тесте через `std::env::set_var` (осторожно: параллельные тесты — серилизуй одним mutex или используй передачу пути аргументом в GhMode::from_path для тестов, env только в prod-обёртке — предпочтительнее: `gh_binary_from_env()` тестируется одним тестом).
2. bump_rev: блок с rev="old" → rev="new"; чужой модуль не тронут; без поля rev — Err или no-op (зафиксируй в тесте).
3. resolve_new_rev с FakeRunner: ls-remote вернул тег с hash X ≠ current → Some(X); == current → None (no-update).
4. write_pending_patch: два файла → на диске .stellwerk/pending/<name>-<epoch>.patch, содержит `### modules.toml (before)`, `(after)`, оба содержимых; возвращает путь.
5. update_module no-gh: GhMode::Missing → пишет patch, возвращает Ok(UpdateOutcome::Pending(path)), audit/measure ФЕЙК (могут быть вызваны до gh — по спецификации: diff готовится после measure; порядок: fetch→build→audit→measure→diff→gh; pending только на gh-шаге). FakeRunner: cargo build Ok, audit Ok, measure Ok; assert Pending + файл существует + ФЕЙК audit/measure были вызваны.
6. update_module с gh (FakeRunner): gh-вызов args содержат `pr create`, `--title`, и НЕ содержат `merge`; outcome PrCreated.
7. build fail (FakeRunner code=1) → Err, gh не зван.
8. audit fail → Err, measure не зван (порядок).
9. no-update (resolve None) → Outcome::NoUpdate, build не зван.

RED: заглушки ломают большинство assert'ов.

## Evidence

- RED wrapper → 0 + вывод.
- STATUS/FILES/TEST/EVIDENCE.
