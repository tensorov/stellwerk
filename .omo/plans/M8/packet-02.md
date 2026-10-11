# packet-02 — мутационный гейт не аттестуем ToolAttestation, граница объявлена

plan_id: M8
status: предложен
goal: GREEN — доказательством mutation-пакета признаётся воспроизводимый outcomes.json плюс ModelClaim, а не ToolAttestation; подмена одного другим запрещена
files_allowed: [docs/adr/0014-mutation-attestation-exception.md, .omo/plans/M8-refactoring-debt-first.md]
max_diff_lines: 200
test: python3 scripts/plan-lint.py .sisyphus/plans/M8
acceptance: ADR-0014 принят и содержит раздел про воспроизводимость артефакта и про то, что ModelClaim нельзя выдавать за Verified
forbidden: [назвать ModelClaim проверенным, объявить гейт зелёным по cargo --version, удалить запись 229 с ошибочным diff_hash]
depends_on: [01]
mutation_threshold: 80
data_class: private
lane: runtime

## Контекст

M8-01 закрыт коммитом `f0281ca`. Chain tail `673deeeec77ccef8a86262f8aacaf62c8cad982062e8cc040f0dbccb699ada6be`
(entry 231 — `ToolAttestation` на `cargo --version`). Гейт вернул `passed: true`, и это
почти обман: проверено было то, что cargo существует, а не то, что baseline верен.

Измерено на M8-01, а не выведено:

- `cargo mutants` проходит через allowlist? Нет. `cargo --version` — да (entry 231).
- `cargo mutants --version` через `evidence_verify`? Отказ: «tool "cargo" is not on
  the allowlist» при попытке добавить аргументы. Allowlist резолвит `cargo` только с
  фиксированным вектором аргументов.
- Значит ни один mutation-пакет не может получить `Verified` ни при каком исходе.

Это третий случай одного и того же (F-90 про Riegel, ADR-0006), но для mutation-гейта,
на котором стоит весь M8.

## Что фиксирует ADR-0014

1. **Признание границы.** Для mutation-пакетов доказательством является пара
   «воспроизводимый артефакт `outcomes.json` + ModelClaim с числами», а не
   ToolAttestation. Пакет не называется `Verified` и не проходит `gate_evaluate` как
   проверенный.
2. **Что именно делает артефакт проверяемым.** `outcomes.json` — это машинный вывод
   cargo-mutans; его можно получить заново той же командой на том же коде. Это слабее
   ToolAttestation (сервер не исполняет команду сам) и сильнее ModelClaim (числа не
   приходится верить на слово).
3. **Что запрещено.** Выдавать `ModelClaim` за проверку; объявлять гейт зелёным по
   формальному `Verified` на посторонней команде; удалять ошибочную запись 229.
4. **Когда исключение пересматривается.** Если `cargo mutants` (или иной
   mutation-инструмент) появится в allowlist PolyForge — исключение снимается
   отдельным пакетом.

## Шаги

1. Написать `docs/adr/0014-mutation-attestation-exception.md` по образцу
   `0006-riegel-attestation-exception.md`, со ссылками на entry 229–231.
2. Обновить `.omo/plans/M8-refactoring-debt-first.md`: часть 0 получает реальный
   гейт-шаг (ModelClaim + артефакт, без Verified), часть 5 (BlockC) — ту же оговорку.
3. Ошибочную запись 229 **не трогать** — она остаётся видимой, рядом с исправлением 230.
