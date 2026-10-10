# packet-15 — исключение аттестации Трека B объявлено ADR, а не оставлено молчанием (F-90)
plan_id: M6
status: выполнен
label: contract-change
goal: ADR-0006 фиксирует, что PolyForge, поднятый внутри Stellwerk, не может аттестовать Riegel, и называет, чем вместо ToolAttestation проверяются пакеты Трека B.
files_allowed: [docs/adr/0006-riegel-attestation-exception.md]
max_diff_lines: 200
test: test -f docs/adr/0006-riegel-attestation-exception.md && grep -q 'ToolAttestation' docs/adr/0006-riegel-attestation-exception.md && grep -q 'evidence_verify' docs/adr/0006-riegel-attestation-exception.md
acceptance: >
  RED — файла нет, `test -f` падает. GREEN — проходит. ADR обязан назвать ЧЕТЫРЕ вещи:
  причину (evidence_verify исполняет команду в каталоге, где нет `polyforge-core`),
  объём (весь Трек B, а не один пакет), замену (вывод `test:` пакета плюс независимое
  ревью) и то, что исключение НЕ тихо. Статус «принят», решение владельца 2026-10-08.
forbidden: [заявлять ToolAttestation для пакетов B, ослаблять требование I3 для пакетов Stellwerk, править docs/adr/0001..0005, объявлять исключение «временным» без срока]
depends_on: []
mutation_threshold: 0
data_class: public
lane: governance

## Почему это ADR, а не строчка в плане

Исключение касается ЗАКОНА 1 («любое „готово“ = ToolAttestation»), то есть контракта
проверки, а не одного пакета. Строчка в `index.md` была бы тихим послаблением: её
не увидит тот, кто открывает ADR-0001 и ищет, что ещё на него опирается.

## Что ADR НЕ делает

Не превращает `ModelClaimed` в нечто среднее и не создаёт «слабый Verified».
`RIEGEL/packet-01` остаётся `ModelClaimed` с честной пометкой; ADR объясняет, почему
иначе нельзя, и называет цену этого ограничения.

## Приёмка

**Коммит реализации:** `b2d5b24`
**Тест пакета:** `test -f docs/adr/0006-riegel-attestation-exception.md && grep -q 'ToolAttestation' docs/adr/0006-riegel-attestation-exception.md` → exit 0 (ВЫПОЛНЕН)
**В main:** да, слит до 5512378.
**Примечание:** `just check` на main → exit 0 после освобождения `/tmp`.
До очистки `/tmp` прогон падал с ENOSPC — следствие F-99, а не пакета.
