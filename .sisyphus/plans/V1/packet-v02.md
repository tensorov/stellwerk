---
id: v02
milestone: V1
status: GREEN
depends_on: [c02]
files_allowed: [crates/stellwerk-task/src/m5/model_transport.rs, crates/stellwerk-task/tests/model_transport_wire.rs, docs/adr/0023-unoffered-tool-calls-are-gate-decisions.md]
overlap_waiver: C9 — N2/00d, N2/00e, N2/00j2, N2/00j3, N2/00j5, N2/00j7 — model_transport.rs/model_transport_wire.rs, последовательное исполнение в одном worktree
acceptance: cargo test -p stellwerk-task --test model_transport_wire
---

# v02 — Вертикальное доказательство отказа

## Цель
Запрещённый вызов даёт `Deny` или `Suspended`, не создаёт кандидат и оставляет названный отказ, потому что отказ, записанный как пустота или как `ToolNotOffered`, неотличим от прогона без решения. `tools` — подсказка модели, а не граница авторизации; решение о вызове принимает только гейт.

## Обоснование (измерено)
- `crates/stellwerk-task/src/m5/model_transport.rs:187-202` — сейчас `ModelProxy` отвечает `ErrorCode::ToolNotOffered` до того, как кадр дойдёт до Pi; скриптованный `bash` через `run_build` даёт `gate=[] requests=0 turns=1 candidate=[]`.
- `crates/stellwerk-task/pi/stellwerk_transport.ts:380-383` — `pi.on("tool_call")` отправляет не-хостовые инструменты (включая `bash`) в гейт; если кадр доходит, отказ становится `GateAnswer::Deny` с причиной R5.
- `crates/stellwerk-cli/src/build_runner.rs:82` — сборка кандидата начинается в workspace, поэтому отказ должен остановить её до появления файлов.
- `crates/stellwerk-task/tests/model_transport_wire.rs:233-261` — два теста сейчас закрепляют `ToolNotOffered`; после contract-change они обязаны закреплять форвардинг до гейта.

## Шаг 1 — падающий тест
`crates/stellwerk-task/tests/model_transport_wire.rs`, тесты `an_explicit_empty_tool_set_refuses_any_call` и `a_call_outside_the_offered_set_is_refused`: сейчас они закрепляют `ErrorCode::ToolNotOffered`; после contract-change они обязаны закреплять форвардинг вызова вне `tools` до клиента. Вертикальное доказательство `bash` перенесено в `V1/packet-v02b.md`, потому что Pi обязан знать удерживаемый инструмент до хука.

## Шаг 2 — минимальная правка
1. Убрать ранний отказ `ToolNotOffered` из `ModelProxy::serve_once`: все `Frame::ToolCall` от backend'а форвардятся клиенту, включая имена вне `tools`.
2. Удалить `ErrorCode::ToolNotOffered` из контракта (или оставить только как исторический вариант, если удаление ломает wire-совместимость; решение и причина — в ADR-0023). Тесты `model_transport_wire.rs` обновить: вызов вне предложенного набора форвардится, `Done { reason: "tool_use" }` появляется, а не `ToolNotOffered`.
3. Вертикальный тест `vertical_v2.rs` не входит в этот пакет: он перенесён в `V1/packet-v02b.md`, потому что Pi 1.0.2 не знает удерживаемый `bash` без регистрации инструмента.
4. ADR-0023 фиксирует, что `tools` — не граница безопасности и что неизвестный/непредложенный вызов идёт в гейт, а не превращается в транспортный отказ.

## Приёмка
cargo test -p stellwerk-task --test model_transport_wire
Ожидается PASS.

## Запрещено
- нельзя трактовать отсутствие ответа как разрешение.
- запрещено создавать кандидат при отказе.
- нельзя записывать отказ как пустоту или как `ToolNotOffered` в обход гейта.
- нельзя ослаблять тесты `model_transport_wire.rs` удалением утверждений; они меняются на новое явное поведение.
- нельзя добавлять `#[allow]`/`#[ignore]` и менять lint-политику.
