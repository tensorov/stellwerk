---
id: v02b
milestone: V1
status: proposed
depends_on: [v02, a02b]
files_allowed: [crates/stellwerk-task/pi/stellwerk_transport.ts, crates/stellwerk-cli/tests/vertical_v2.rs]
limit_waiver: C2 — tests/vertical_v2.rs 301 строка; ADR-0023 разрешает тест сверх tests-бюджета 250
overlap_waiver: C9 — a02b/stellwerk_transport.ts, последовательное исполнение в одном worktree
acceptance: cargo test -p stellwerk-cli --test vertical_v2
---

# v02b — Pi держит `bash`, но не предлагает его модели

## Цель
Довести контракт v02 до исполняемого состояния: `ModelProxy` уже форвардит
`Frame::ToolCall` клиенту, а Pi обязан знать удерживаемый `bash`, чтобы хук
`pi.on("tool_call")` мог отправить его в гейт. Без регистрации инструмента Pi
1.0.2 отвечает `Tool bash not found` до хука, и отказ снова становится пустотой,
неотличимой от прогона без решения.

## Обоснование (измерено)
- `crates/stellwerk-cli/src/build_runner.rs:80` — `BUILD_OFFERED` содержит только
  `read`, `edit`, `write`; это правильно: модель не должна видеть `bash`.
- `crates/stellwerk-cli/src/build_runner.rs:85` — `BUILD_HELD` уже содержит
  `bash`; состояние «держим, но не предлагаем» объявлено, но не реализовано в Pi.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:322-374` — extension
  регистрирует только `read`, `edit`, `write`; `bash` не попадает в реестр Pi.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:376-379` — `pi.on("tool_call")`
  отправляет в гейт все не-хостовые инструменты; ветка для `bash` уже есть, но
  Pi не доходит до неё.
- RED 2026-11-02: `cargo test -p stellwerk-cli --test vertical_v2` →
  `Tool bash not found`, `gate=[] requests=0 turns=2 candidate=[]`.

## Шаг 1 — падающий тест
`crates/stellwerk-cli/tests/vertical_v2.rs`, тест
`a_forbidden_build_call_is_denied_and_leaves_no_candidate`: после `bash` от
скриптованной модели гейт обязан назвать `GateAnswer::Deny { tool: "bash",
reason: R5 }`, `gate_requests > 0`, кандидат пуст, `Succeeded` нет. Сейчас тест
падает на `gate=[] requests=0`, потому что Pi не знает `bash`.

## Шаг 2 — минимальная правка
1. В `stellwerk_transport.ts` зарегистрировать `bash` через `defineTool` как
   удерживаемый инструмент с параметром `command`.
2. `execute` этого инструмента **не запускает оболочку** и не пишет файлы: он
   возвращает названный отказ (`held_tool_unreachable`), если хук почему-то не
   заблокировал вызов. Единственный путь выполнения — `pi.on("tool_call")` →
   гейт; `Allow` для R5 не должен возникать.
3. Не добавлять `bash` в `BUILD_OFFERED`, `HOST_TOOLS` или промпт: модель
   по-прежнему видит только `read`, `edit`, `write`; удержание — не предложение.
4. Оставить вертикальный тест как есть; он доказывает, что заявка дошла до гейта
   и отказ назван.

## Приёмка
cargo test -p stellwerk-cli --test vertical_v2
Ожидается PASS, включая `a_forbidden_build_call_is_denied_and_leaves_no_candidate`.

## Запрещено
- нельзя добавлять `bash` в `BUILD_OFFERED`/промпт: это обещание, которого
  политика не выполнит.
- нельзя исполнять команду в `execute`; `bash` остаётся удерживаемым, а не
  рабочим инструментом.
- нельзя обходить `pi.on("tool_call")` или считать отсутствие ответа разрешением.
- нельзя править `model_transport.rs`/`build_runner.rs`: это области v02/a02b.
- нельзя добавлять `#[allow]`/`#[ignore]` и менять lint-политику.
