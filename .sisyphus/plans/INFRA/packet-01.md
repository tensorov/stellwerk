# packet-01
plan_id: INFRA
goal: Создать крейт `xtask` — захардкоженный перечень задач верификации, чтобы `evidence_verify` мог доказать не-cargo наборы.
files_allowed: [Cargo.toml, .cargo/config.toml, xtask/Cargo.toml, xtask/src/main.rs]
max_diff_lines: 200
limit_waiver: xtask не укладывается в 3 файла по природе задачи: отдельный Cargo.toml крейта плюс корневой Cargo.toml (contract-change) плюс .cargo/config.toml (алиас cargo xtask) плюс исходник
test: cargo xtask plan-lint
acceptance: `cargo xtask plan-lint` и `cargo xtask worker-tests` запускают соответствующие наборы и наследуют их код возврата. `cargo xtask любая-другая-строка` обязан завершиться с ненулевым кодом, НИЧЕГО не исполнив и не выведя подсказку с путями. Лишние аргументы (`cargo xtask worker-tests --что-угодно`) НЕ перенаправляются в исполняемую команду.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки, проброс аргументов в задачу, параметризация команды путём]
depends_on: [—]
data_class: public
lane: infra/xtask
Contract-Change: root Cargo.toml [workspace].members += "xtask"
Reason: allowlist evidence_verify принимает только cargo-семейство, поэтому bash-наборы (34 теста scripts/worker) и python-линтер неаттестуемы (F-08). xtask — обычный крейт workspace, его содержимое под контролем версий и ревью
Note: ADR не требуется — правило «contract-change + ADR» в AGENTS.md касается ЛИНТ-политики, а не состава workspace. Прецедент: M3/packet-01 добавлял members тем же способом
Depends-On-External: F-08

## Контекст

- `evidence_verify` отклоняет `bash`, `just`, `just check` («not on the allowlist»).
  Из-за этого блок `worker-pipeline-hardening` остался в состоянии
  `ModelClaimed` без `Verified`, и `gate_evaluate` вернул
  `passed: false, missing: ["Verified"]`.
- Расширять allowlist до `bash` нельзя: команда с произвольными аргументами
  исполнит что угодно, и allowlist перестанет быть allowlist'ом. PolyForge
  разделяет ModelClaim и ToolAttestation именно потому, что модель не может
  верифицировать себя.
- `xtask` — паттерн, а не библиотека: достаточно `std::process::Command`.
  Зависимостей нет, `[workspace.dependencies]` не меняется.

## Безопасность xtask (главное требование пакета)

`xtask` входит в доверенную базу: он — мост к allowlist PolyForge. Поэтому
задачи обязаны быть **литеральным перечислением**, а не командой из аргумента.

1. `match` по строковому литералу. Никакого `Command::new(из_аргумента)`.
2. Аргументы, кроме имени задачи, **не перенаправляются**. Иначе
   `cargo xtask worker-tests --exec=что-то` снова откроет произвольное
   исполнение.
3. Неизвестная задача → сообщение со **списком разрешённых имён** и ненулевой
   код. Не показывать пользовательский ввод в команде и не предлагать «похожие»
   имена, которые можно выстрелить.
4. Путь к набору задан константой в исходнике, не аргументом.

## Что сделать

1. `xtask/Cargo.toml`: крейт без зависимостей, `edition = "2021"`.
2. `xtask/src/main.rs`: перечисление задач:
   - `worker-tests` → `bash scripts/worker/tests/run.sh`
   - `plan-lint` → `python3 scripts/plan-lint.py`
   - `plan-lint-selftest` → `python3 scripts/plan-lint.py --self-test`
   Код возврата дочернего процесса пробрасывается как есть.
3. `Cargo.toml`: добавить `"xtask"` в `[workspace].members`.
4. `.cargo/config.toml`: `[alias] xtask = "run --package xtask --"`. **Без этого
   пакета шаг не выполним:** конвенция `cargo <name>` ищет внешний бинарник
   `cargo-xtask` в PATH, а крейт живёт в workspace и не устанавливается. Первый
   прогон без алиаса дал `error: no such command: xtask` — это и было
   упущением пакета, а не ошибкой кода.

## Evidence

- `cargo xtask plan-lint` → код возврата плана (0, если нарушений нет).
- `cargo xtask worker-tests` → 34/34.
- `cargo xtask несуществующая` → ненулевой код, ничего не исполнив.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
