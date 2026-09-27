# packet-06
plan_id: M1
goal: Добавить тест и реализацию `RiskClass::required_level` — единственный предикат контракта, оставшийся без теста.
files_allowed: [crates/stellwerk-core/src/contract.rs, crates/stellwerk-core/tests/risk_level_policy.rs]
max_diff_lines: 200
test: cargo test -p stellwerk-core --test risk_level_policy
acceptance: Новый тест сначала падает на `not yet implemented: RED: policy::required_level` (зафиксировать вывод), затем после реализации проходит. Итог по крейту: 21 passed, 27 failed.
forbidden: [cargo add, cargo install, правка тестов, #[ignore], #[allow], ослабление assert, refactor, "попутные" правки]
depends_on: [01, 02, 03, 04, 05]
parallelism: sequential-only (single file lock — contract.rs)
data_class: public
lane: m1/core-risk

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

**Особенность пакета:** `required_level` — единственный предикат, под который
ночная сессия не написала тест. Это осознанный долг, и он закрывается здесь.

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

Лестница, согласованная с `rbac_level`:
- R0 → L0Local (только чтение);
- R1 → L2Contributor (запись);
- R2 → L3Maintainer (сеть);
- R3 → L4Admin (секреты);
- R4 → L5Owner (внешний наблюдаемый эффект);
- R5 → L5Owner (необратимо).

1. RED: создать `tests/risk_level_policy.rs` — таблица выше, проверка каждой пары плюс
   монотонность на соседних уровнях (R0 < R1, R4 == R5). Убедиться, что тест ПАДАЕТ,
   зафиксировать вывод.
2. GREEN: `match self` с возвратом `PolicyLevel`.
3. Удалить `let _ = self;`.
4. `PolicyLevel::permits` не трогать — он в packet-05.

## Evidence

- Сначала RED-вывод нового теста, затем `cargo test -p stellwerk-core --test risk_level_policy` → passed.
- Формат ответа: `STATUS: DONE|BLOCKED` / `FILES:` / `TEST:` / `EVIDENCE:` (по `AGENTS.md`).
- Коммит не делать без явного указания владельца.
