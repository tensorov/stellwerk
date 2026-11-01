---
id: N2/00i1
milestone: N2
title: Кадр гейта несёт путь, политика может отказать по основанию
depends_on: [N2/00h3e, 00e, 00f]
allowed_changes:
  - crates/stellwerk-task/src/m5/gate_channel.rs
  - crates/stellwerk-task/pi/stellwerk_transport.ts
  - crates/stellwerk-task/tests/gate_channel.rs
---

# 00i1 — путь в кадре гейта

## Зачем

`GateRequest` не несёт аргументов, поэтому политика не может отличить правку
`src/lib.rs` от правки `../../etc/passwd`: она видит только имя инструмента.
Второй дефект: политика умеет сказать «нет данных» (`None` ⇒ `Unclassified`),
но не «запрещено планом» — отказ политики неотличим от отсутствия ответа.

## Что делается

1. `GateRequest.path: Option<String>` (`#[serde(default)]`), `"path"` в белый
   список `parse_request`. Кадр старого клиента без пути — `None`.
2. `AllowBasis { RiskTable, ApprovedPlan { scope } }` и
   `PolicyVerdict { Evaluated, Unclassified, Refused { reason } }`.
3. `GatePolicy::decide_call(call, path)` с умолчанием: путь пришёл, а политика
   его не толкует ⇒ `Refused { "policy_does_not_handle_paths" }`. Умолчание не
   пропускает неразобранный путь.
4. `GateAnswer::Authorized { tool, scope }` — разрешение таблицы, у которого
   названо основание. База только аннотирует `Allow`, не подменяет `Ask`/`Deny`.
5. `serve_once_logged` возвращает ответ рядом со статусом; `serve_once` делегирует.
6. Pi-адаптер читает `event.input.path` и шлёт `path`; `authorized` не блокирует.

## RED

- `a_frame_carries_the_path_to_the_policy`
- `a_frame_without_a_path_leaves_it_unknown`
- `a_policy_that_did_not_opt_into_paths_refuses_them`
- `a_policy_denial_is_deny_not_unclassified`
- `an_approved_plan_basis_is_authorized_not_allow`
- `a_plan_basis_does_not_override_ask`
- `a_self_assigned_risk_is_refused_before_the_policy` остаётся зелёным.

## Не делается

Скоуп путей и записи `edit`/`write` в реестре — 00i2. Валидация
`allowed_changes` — 00i3. `bash` остаётся R5.
