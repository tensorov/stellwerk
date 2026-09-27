# Stellwerk Federation — инварианты

## Закон
1. Модель не верифицирует себя. Любое «готово» = ToolAttestation от PolyForge, не текст.
2. TDD: сначала падающий тест, потом код. Коммит без нового теста — отклоняется.
3. Каждая задача = один task packet. Полный план читает только Atlas.
4. Не расширяй скоуп. Нашёл смежную проблему — заведи FINDING, не чини.
5. Не добавляй зависимости. Разрешены только крейты из [workspace.dependencies]
   корневого Cargo.toml (подключать через `workspace = true`). Нужен новый — верни BLOCKED.

Lint-политика (исключения в `[workspace.lints.clippy]` или `[lints.clippy]`
крейта) меняется только packet'ом с меткой `contract-change` **и** ADR в
`docs/adr/`: `#[allow(...)]` в коде запрещён безусловно.

## Границы правки
- ≤ 3 файла и ≤ 200 строк diff на один task packet. Больше — верни SPLIT_REQUIRED.
- Не трогай: modules.lock, .github/workflows/, crates/stellwerk-core/src/contract.rs,
  корневой Cargo.toml (только через отдельный packet с меткой contract-change).

## Протокол завершения
RED   -> покажи вывод падающего теста
GREEN -> покажи вывод проходящего теста
PROOF -> evidence_append(ModelClaim) + evidence_verify(cmd=<точная команда теста>)
GATE  -> gate_evaluate; если fail — не отчитывайся об успехе

## Формат ответа (последний блок, всегда)
STATUS: DONE | BLOCKED | SPLIT_REQUIRED | FINDING
FILES: <список>
TEST: <точная команда>
EVIDENCE: <chain tail hash>

## Запрещено
Правки без теста, «рефакторинг заодно», подавление предупреждений, #[allow(...)],
#[ignore], skip тестов, мок вместо реализации без пометки в packet, cargo add.
команды, печатающие resolved config или секреты (opencode debug config,
cat auth.json, env | grep KEY и подобные).
