# packet-05
plan_id: M1
goal: Реализовать `PolicyLevel::permits` — лестницу полномочий федерации.
files_allowed: [crates/stellwerk-core/src/contract.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test rbac_level
acceptance: `cargo test -p stellwerk-core --test rbac_level` → 11 passed; 0 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01, 02, 03, 04]
parallelism: sequential-only (single file lock — contract.rs)
data_class: public
lane: m1/core-rbac

## Контекст

- Крейт `stellwerk-core`, единственный файл контракта — `src/contract.rs`.
- Контракт заморожен: правится ТОЛЬКО предикат, названный в этом пакете. Любая другая
  правка `contract.rs` — нарушение packet'а.
- Предикаты объявлены с телом `async { todo!(...) }` и строкой `let _ = (…)`, чтобы
  `clippy -D warnings` оставался зелёным. При реализации `let _ = …` УДАЛЯЕТСЯ.
- RED-тесты написаны ночной сессией (фаза 3) в `crates/stellwerk-core/tests/`.
  Их нельзя править: они приказ, а не предложение.
- Правило проекта: `todo!()` лучше неверного ответа — пока предикат не реализован,
  вызов падает loudly.

## Единая таблица `risk_class` — источник истины

Этот раздел **буквально идентичен** в `packet-05.md` и `packet-06.md`. Причина:
две таблицы измеряют разные единицы, и без явного правила композиции они
противоречат друг другу (см. «Конфликт, который закрывает этот раздел»).

### Таблица (единая для обоих пакетов)

| risk_class | min_policy_level | verdict | смысл |
|---|---|---|---|
| R0 | `L0Local` | Allow | чтение, эффекта нет |
| R1 | `L2Contributor` | Allow | запись в песочницу, откатывается её удалением |
| R2 | `L3Maintainer` | Ask | сетевой выход: наружу уходят данные |
| R3 | `L4Admin` | Ask | доступ к секретам: решает человек |
| R4 | `L5Owner` | Deny | внешний наблюдаемый эффект без ведома владельца |
| R5 | `L5Owner` | Deny | необратимое разрушение |

Столбец `verdict` совпадает с лестницей `permission_for` из
`.sisyphus/plans/M3/packet-02.md` (R0–R1 Allow, R2–R3 Ask, R4–R5 Deny). Расхождений нет:
M1 задаёт полномочия и вердикт по риску, M3 превращает их в решение шлюза.

### Таблица `Action → min PolicyLevel` (вторая ось)

| action | min PolicyLevel |
|---|---|
| `ReadWorkspace` | `L0Local` |
| `WriteWorkspace` | `L2Contributor` |
| `RunTask` | `L2Contributor` |
| `UpdateModule` | `L3Maintainer` |
| `ManageMembers` | `L4Admin` |
| `GrantRoles` | `L5Owner` |

### Правило композиции (обязательно, иначе таблицы противоречат друг другу)

```
effective_level   = max(permits_min(action), risk_table[risk].min_policy_level)
effective_verdict = Deny, если любой из двух вердиктов == Deny
                    иначе Ask, если любой == Ask
                    иначе Allow
allow             = (caller >= effective_level) AND (effective_verdict == Allow)
```

`Deny` **абсолютен**: не эскалируется уровнем, даже `L5Owner`. Владелец может
разрешить конкретное действие явно, но это отдельный вызов, а не следствие таблицы.

### Конфликт, который закрывает этот раздел

`permits_min(RunTask) = L2Contributor`, но `risk_table[R2].min_policy_level = L3Maintainer`.
Исполнитель с уровнем `L2Contributor` удовлетворяет проверке действия и **не**
удовлетворяет проверке риска. До появления этого раздела ни один пакет не говорил,
кто побеждает, — воркер, реализовавший только `packet-05`, построил бы таблицу,
которая молча разрешает сетевой вызов при `L2`. Ответ: побеждает **оба условия**
(логическое И), то есть эффективный порог `max(L2, L3) = L3`.

### Инварианты (проверяемые тестами)

- **INV-RB-1** таблица риска полна: ровно 6 строк на 6 вариантов `RiskClass`;
- **INV-RB-2** `risk_table[r].min_policy_level` не убывает при росте `r`;
- **INV-RB-3** `R4` и `R5` имеют `verdict == Deny` при любом уровне;
- **INV-RB-4** `permits_min(action)` монотонна и не превышает `L5Owner`;
- **INV-RB-5** таблицы ортогональны: правка любой из них требует перепроверки
  другой и правки этого раздела в обоих пакетах одновременно.

## Что сделать

Лестница прав из `tests/rbac_level.rs`:
- `ReadWorkspace` — всем, начиная с `L0Local`;
- `L0Local` НЕ может `WriteWorkspace` и НЕ может `RunTask`;
- `L2Contributor`: WriteWorkspace, RunTask; НЕ UpdateModule/ManageMembers/GrantRoles;
- `L3Maintainer`: + UpdateModule; НЕ ManageMembers/GrantRoles;
- `L4Admin`: + ManageMembers; НЕ GrantRoles;
- `GrantRoles` — ТОЛЬКО `L5Owner`: иначе скомпрометированный админ тихо расширит права;
- монотонность: если право есть у уровня, оно есть у всех верхних.

1. Таблица «действие → минимальный уровень», затем `self >= required`. Таблица
   гарантирует монотонность конструктивно.
2. `GrantRoles` → `PolicyLevel::L5Owner`.
3. Удалить `let _ = (self, action);`.

## Evidence

- `cargo test -p stellwerk-core --test rbac_level` → 11 passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
