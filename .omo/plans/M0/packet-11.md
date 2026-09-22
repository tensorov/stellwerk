# packet-11
plan_id: M0
goal: update GREEN — полный flow (fetch→build→audit→measure→gh pr create / pending), wiring CLI update, зелёные тесты packet-10.
files_allowed: [crates/stellwerk-registry/src/cmd_update.rs, crates/stellwerk-registry/src/cli.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-registry update
acceptance: cargo test -p stellwerk-registry update → exit 0 (все тесты 10 + CLI-тесты); сборка clippy чисто; ручной: в tempdir с фейковым gh-скриптом (STELLWERK_GH_BIN=fixture) exit 0; с несуществующим бинарём — exit 0 + pending-файл; реальный gh НЕ вызывается в автотестах (только в финальной ручной M0-приёмке).
forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock/modules.toml руками в тестах репо, правка contract.rs, правка lib.rs/model.rs, реальный gh/git/cargo network в тестах, merge PR, #[ignore], #[allow], ослабление тестов packet-10]
depends_on: [packet-10]

## GREEN-пакет

Финальный packet M0 (lane-update). Зелёные тесты 10 + cli dispatch. Поправка D — verbatim в коде/тестах:

> update reads binary from env STELLWERK_GH_BIN (default `gh`); tests inject fixture script; missing-binary scenario → .stellwerk/pending/ + stop covered by test with nonexistent bin; real gh only in manual final M0 acceptance. Pending dir is `.stellwerk/pending/` (NOT legacy `.nexus/pending/`).

## Что сделать (cmd_update.rs)

1. Реализуй оркестратор (тесты определяют контракт 10 — не ослабляй):
   - `resolve_new_rev` через runner: `git ls-remote --tags <url>`; parse lines `hash\trefs/tags/x`; channel=tag → **первые строка** вывода, где ref НЕ заканчивается на `^{}` (annotator-суффикс); её hash = new rev. Детерминированно: не «самый свежий», не semver-сортировка (крейтов для этого нет) — просто первая не-`^{}`-строка, как её отдаёт FakeRunner/ls-remote. Сравни с current rev → None если equal.
   - Self-build: runner.run("cargo", ["build","-p","stellwerk-registry"], root) — code≠0 → Err.
   - audit_fn(root, name)?, measure_fn(root, name)? — инжектированы, порядок строго после build.
   - diff: исходные строки modules.toml/lock до изменений (rev bump + measure) vs после; если изменений нет → NoUpdate (ранний выход можно и до build, если resolve None).
   - gh: `gh_binary_from_env()`; Binary(path) → runner.run(path, ["pr","create","--title",title,"--body",body,"--base",default_branch?]) — base можно опустить (gh сам default); **ни один вызов не содержит "merge"**. Missing → write_pending_patch → Pending(path) → печать `pending: {path}`.
2. `pub fn update_module(root: &Path, name: &str, runner: &dyn CommandRunner, audit: &dyn Fn(&Path,&str)->Result<(),UpdateError>, measure: &dyn Fn(&Path,&str)->Result<(),UpdateError>, gh: GhMode, epoch: u64) -> Result<UpdateOutcome, UpdateError>` — epoch инжектируется для детерминированных путей pending (prod: SystemTime).
3. cli.rs dispatch Update(NameArgs): match-arm в КОНЕЦ существующего match; name обязателен; prod-wiring: RealRunner, `|r,n| cmd_audit::audit_module_cli(...)`, measure через cmd_measure, gh_binary_from_env(), epoch=now; exit 0 для NoUpdate/Pending/PrCreated, 1 для Err. Печать человекочитаемого статуса.

## Тесты (добавить; старые 10 не ослаблять)

- Все тесты packet-10 зелёные.
- CLI: отсутствующий модуль (нет modules.toml / нет имени) → exit 1 — этого достаточно для CLI-теста 11 (help/usage уже в 04); НЕ зв `update_module` из CLI-теста с реальным временем. Все тесты `update_module` передают фиксированный epoch (напр. 1_234_567_890) для детерминированных pending-путей. Один интеграционный прогон через `update_module` с RealRunner **не** звать — только fake.
- Патч-формат: точный `### modules.toml (before)\n...\n### modules.toml (after)\n...`.

## Приёмка

- `cargo test -p stellwerk-registry update` → 0.
- `cargo test --workspace` → 0 (все lanes смержены).
- `cargo clippy --workspace --all-targets -- -D warnings` → 0.
- Ручной smoke (опционально, не в тестах): STELLWERK_GH_BIN=/nonexistent target/debug/stellwerk module update … → pending.
- После merge packet-11: M0 код готов; финальная M0-приёмка (F4) — вне packets, руками: реальный polyforge 813±10%, реальный gh pr create по желанию.

## Evidence

- GREEN вывод update + workspace tests; PROOF evidence_verify(cmd='cargo test -p stellwerk-registry update'); GATE; STATUS/FILES/TEST/EVIDENCE.
