# packet-01
plan_id: M3
goal: Создать крейт `stellwerk-gateway` и падающий тест отображения RiskClass → permission. contract-change: добавляется член workspace.
files_allowed: [Cargo.toml, Cargo.lock, crates/stellwerk-gateway/Cargo.toml, crates/stellwerk-gateway/src/lib.rs, crates/stellwerk-gateway/src/permission.rs, crates/stellwerk-gateway/tests/permission.rs]
limit_waiver: пакет создаёт крейт: Cargo.lock генерирует cargo, плюс отдельные модули permission/gate. Скелет крейта не укладывается в 3 файла по природе задачи.
max_diff_lines: 200
test: cargo test -p stellwerk-gateway --test permission && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings
acceptance: RED: `bash -c 'cargo build -p stellwerk-gateway && cargo test -p stellwerk-gateway --test permission --no-run && ! cargo test -p stellwerk-gateway --test permission'` → exit 0: собирается, тест компилируется, падает на assert.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [—]
data_class: public
lane: m3/skeleton
Contract-Change: root Cargo.toml [workspace].members += "crates/stellwerk-gateway"
Reason: создаётся новый член workspace; AGENTS.md запрещает трогать корневой Cargo.toml без packet'а с меткой contract-change
Note: ADR не требуется — правило «contract-change + ADR» в AGENTS.md касается ЛИНТ-политики, а не состава workspace

## Контекст

- **Новый крейт `stellwerk-gateway`** — виртуальный MCP-сервер: он НЕ выполняет
  инструменты сам, а решает, что можно, спрашивает, когда надо, и пишет чек.
- Крейт зависит от `stellwerk-core` и `stellwerk-store` по path. Новых крейтов нет;
  `cargo add` запрещён.
- Решения принимаются ТОЛЬКО через предикаты контракта:
  `RiskClass::required_level`, `PolicyLevel::permits`, `DataClass::permits_model`
  (все реализованы в M1, packet-05/06/04). Своих копий правил не заводить.
- Схемы MCP-модулей уже измерены: `measure` даёт `context_cost_tokens` (polyforge = 813).
  Gateway решает, что монтировать, по этим числам, а не по вкусу.
- Ключи и секреты не читать и не логировать: `auth.json`, `models.json`, `env | grep KEY`
  запрещены (`AGENTS.md`).

## Что сделать

**contract-change:** правка корневого `Cargo.toml` — только добавление члена `members`.

1. Корневой `Cargo.toml`: `crates/stellwerk-gateway` в `members`. Больше ничего.
2. `Cargo.toml` крейта: `stellwerk-core`, `stellwerk-store` по path; из workspace —
   `serde`, `serde_json`, `thiserror`, `sha2`. `clap` и `tokio` — по необходимости.
3. `src/permission.rs`:
   - `pub enum Permission { Allow, Ask, Deny }` (сериализуется lowercase);
   - `pub fn permission_for(risk: RiskClass) -> Permission` — **RED-заглушка**;
   - `PermissionError` (thiserror) с `NotImplemented`.
4. Тесты в `tests/permission.rs` — лестница решений:

| RiskClass | Permission | почему |
|---|---|---|
| R0 | Allow | чтение, эффекта нет |
| R1 | Allow | запись в песочницу, откатывается её удалением |
| R2 | Ask | сетевой выход: наружу уходят данные |
| R3 | Ask | доступ к секретам: решает человек |
| R4 | Deny | внешний наблюдаемый эффект без ведома владельца |
| R5 | Deny | необратимое разрушение |

   Плюс: `permission_for` для всех шести классов проверяется таблицей, а не
   перечислением — иначе добавление R6 в контракт не сломает тест молча.
5. Реализацию НЕ пиши — это packet-02.

### Утверждённая таблица вердиктов (обязательна, не на усмотрение воркера)

Тест RED обязан утверждать **ровно** эту таблицу. Она выведена из решений
владельца В1 (2026-09-27), а не скопирована из REVIEW.md:

| risk | permission | основание |
|---|---|---|
| R0 | `Allow` | M1-06, без изменений |
| R1 | `Allow` | M1-06, без изменений |
| R2 | `Ask` | M1-06, без изменений |
| R3 | `Ask` | M1-06, без изменений (уровень стал `L5Owner` — ADR-0002) |
| **R4** | **`Ask`** | **В1: был `Deny`. Изменён.** |
| R5 | `Deny` | В1: безусловный, не suspend'ится никогда |

**Почему у R4 именно `Ask`, а не `Deny`.** `permission_for` принимает только
`RiskClass` — уровня в сигнатуре нет. Решение «R4 при `L4Admin` и ниже → `Deny`»
принимает **уровень**, и это делает `gate` в M3-04, а не эта таблица. Если
здесь написать `Deny`, то R4 станет недостижим для владельца: `Deny`
абсолютен и по В1 не эскалируется уровнем, значит одобрение не спасло бы вызов.
Согласованность между таблицей и уровнем проверяется в M3-04; здесь она была бы
проверкой не той оси.

**Ловушка, которую этот раздел закрывает.** `packet-02` содержит в `forbidden`
«правка тестов». Если тест из этого пакета утверждает `R4 → Deny`, то воркер
M3-02 окажется перед двумя взаимоисключающими требованиями: реализовать В1 или
не трогать тест. Оба нарушения. Поэтому правильная таблица должна появиться
здесь, в пакете-источнике, а не появляться правкой теста в следующем пакете.

### Обязательный самопровер перед завершением

Postflight (`cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets
-- -D warnings`) прогоняется ПОСЛЕ того, как воркер завершился. У воркера нет
второго шанса: он не может починить дефект, который заметит гейт. Поэтому
прогнать обе команды обязан сам воркер, ДО отчёта:

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Первая форма **мутирующая** — приводит код к канону. Вторая только проверяет и
должна завершиться без вывода. Если после `cargo fmt` clippy всё ещё ругается
(например, `unused variable` — это ошибка компиляции, а не формат), это не
«попутная правка», а недоделка пакета: исправь и повтори прогон.

Обе команды обязательны и в `TEST:`, и в отчёте: их вывод — часть evidence.

## Evidence

- RED-вывод `cargo test -p stellwerk-gateway --test permission`.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
