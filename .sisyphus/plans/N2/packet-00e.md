# N2-00e — канал авторизации инструмента (хостовая сторона)

**label:** — · **mutation_gate:** yes (граница «до эффекта»)
**depends_on:** [N2-00d, 00d] · **ADR:** не требуется
overlap_waiver: C9 — N1 и N2 делят crates/stellwerk-task/src/m5/mod.rs, последовательное исполнение

## Цель

Песочница перед эффектом спрашивает хоста: «можно ли вызвать инструмент `X` из этой
сессии, и не вложенный ли это вызов?». Хост отвечает вердиктом, в котором
**запрещено политикой**, **спросить было некого**, **о риске нет данных**, **вызов не
опознан** и **разрешено** — разные состояния. Ни одно из них не выглядит как `allow`.

Инвариант:

> Классификацию риска и решение принимает ТОЛЬКО хост. В кадре заявки нет ни `risk`,
> ни `module`, ни `caller`, ни `data`, ни `url`: песочница не может выписать себе
> разрешение. Кадр с лишним полем отвергается до вызова политики. `suspended` и
> `deny` различимы в статусе и в evidence (В4).

Второй сокет (`gate.sock`) рядом с `model.sock`, а не мультиплекс: у модели «один ход на
соединение», у гейта «один вердикт на соединение», и общий сокет потребовал бы
супервизора, которого в этом пакете нет.

## production_call

- `GateChannel::bind` / `serve_once` — **библиотечный шов**: реальную классификацию риска
  даёт registry (`modules.lock`) в N2-00g, блокировку эффекта до ответа — Pi-extension в
  N2-00f. Живого вызывающего в этом пакете нет; это заявлено явно, чтобы «нет
  вызывающего» не читалось как «уже в продакшене».

## files_allowed

- `crates/stellwerk-task/src/m5/gate_channel.rs` (новый)
- `crates/stellwerk-task/src/m5/mod.rs`
- `crates/stellwerk-task/tests/gate_channel.rs` (новый)

**В лимите ADR-0003 (3 файла).**

## Что делать

1. `GateChannel::bind(dir)` создаёт `gate.sock` в хозяйском каталоге, отдаёт `socket()` и
   удаляет файл в `Drop` — как `ModelProxy` в 00d.
2. Кадр клиент → хост: `{"kind":"tool_call","tool":"read","parent_tool_call_id":null}`.
   Разрешены ровно три ключа; лишний (`risk`, `url`, `module`, …) → `unexpected_field` с
   именем поля; неизвестный `kind` → `unknown_kind`.
3. Проверка прав — существующим `SessionDescription::adjudicate(tool, parent)`:
   `NoSuchTool` → `unknown_tool` с текущим набором, `UnrecognisedSource` (пустой
   `parentToolCallId`) → `unrecognised_source`. В обоих случаях политика **не вызывается**.
4. Порт хостовой политики (не провод):

   ```rust
   pub trait GatePolicy: Send + Sync {
       fn request_for(&self, call: &Adjudication) -> Option<CallRequest>;
   }
   ```

   `Some(req)` → канал сам зовёт `stellwerk_gateway::gate::decide(&req)`; своей таблицы
   рисков здесь нет. `None` → `unclassified`: отсутствие данных — отдельное состояние,
   не `allow`.
5. Ответы (`kind`, ровно один на соединение): `allow`, `ask`, `suspended`,
   `deny{reason}`, `unknown_tool{held}`, `unrecognised_source`, `unclassified`,
   `refused{code,message}` (`unknown_kind | malformed_json | unexpected_field | schema |
   oversize_frame | identity`). `Served::{Answered, NoRequest, Refused}` — три состояния,
   не `bool`; пустое соединение — `NoRequest`, а не успех.
6. `MAX_FRAME_BYTES` переиспользуется из `model_transport`: второй предел стал бы вторым
   источником истины. Превышение отвергается до буферизации всего кадра; хвост
   дочитывается до EOF, иначе закрытие с непрочитанными байтами шлёт RST и клиент не
   увидит отказ.
7. Тесты (`tests/gate_channel.rs`), 10 штук:
   - `an_allowed_tool_is_answered_allow` — R0 → `allow`, политика спрошена ровно раз;
   - `a_policy_deny_is_deny_not_suspended` — R5 → `deny` с причиной;
   - `an_unknown_answer_is_never_allow` — R2 без интерактива → `suspended`; политика без
     риска (`None`) → `unclassified`; ни один неизвестный случай не `allow`;
   - `a_call_outside_the_tool_set_never_reaches_the_policy` — `unknown_tool`, политика 0;
   - `an_empty_parent_is_unrecognised_source` — `""` → `unrecognised_source`, политика 0;
   - `a_nested_call_keeps_its_parent` — политика видит `CallSource::Nested`;
   - `a_self_assigned_risk_is_refused_before_the_policy` — поле `risk` в заявке →
     `unexpected_field`, политика 0;
   - `a_second_request_is_not_served` — ровно один ответ на соединение;
   - `a_connection_without_a_request_is_no_request` — `NoRequest`, кадров нет;
   - `an_oversize_gate_frame_is_refused` — > 64 KiB → `oversize_frame`.
8. Мутации: 18 мутантов, 10 caught / 2 missed / 6 unviable (83% при пороге 75%).
   Выжили два: `Drop for GateChannel` → `()` (удаление сокета при дропе не проверено —
   нет строки бюджета на тест) и `read_request: > → >=` (граница ровно `MAX_FRAME_BYTES`
   не проверена: нужен кадр размером точно в лимит). Оба — известные пробелы, не
   маскирующие разрешение по умолчанию.

## Почему `mutation_gate: yes`

Мутант, который на `None` от политики отвечает `allow`, на `Suspended` отвечает `deny`
или `allow`, пропускает заявку с `risk`/`url` до политики или обслуживает вторую заявку,
превращает гейт в разрешение по умолчанию или в неотличимый отказ.

## Запрещено трогать

`model_transport.rs`, `sandbox_bwrap.rs`, `secrets.rs`, `orchestrator.rs`, `runner.rs`,
`session.rs` gateway, `RISK_PERMISSION`, `evidence.rs`, store, `justfile`, `.github/`,
`scripts/plan-lint.py`, корневой `Cargo.toml`, `modules.lock`. Новых зависимостей нет:
`stellwerk-gateway` и `stellwerk-core` уже в `Cargo.toml`; сеть не используется.

## Приёмка

```
cargo test -p stellwerk-task --test gate_channel
cargo test -p stellwerk-task
cargo build --workspace --all-targets
cargo clippy -p stellwerk-task --all-targets --no-deps -- -D warnings
cargo fmt -p stellwerk-task -- --check
cargo mutants -p stellwerk-task --file crates/stellwerk-task/src/m5/gate_channel.rs -- --test gate_channel
```

C2: src ≤ 200 логических строк, tests ≤ 250 строк, ровно 3 файла.
