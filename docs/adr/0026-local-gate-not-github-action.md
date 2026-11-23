# ADR-0026: Гейт PolyForge локальный — GitHub Actions не может его выполнить

Статус: принят
Дата: 2026-11-23
Пакет: publication-readiness / task-20
Связано: ADR-0027 (CI зеркалирует локальный строгий гейт как tier-1);
`scripts/integrator/gate.sh`; `crates/stellwerk-cli/tests/ci_gate.rs`

## Проблема

Workflow `polyforge` в `.github/workflows/` вызывал
`scripts/integrator/gate.sh` с `${{ github.head_ref }}` в качестве аргумента.
Этот workflow был мёртвым кодом:
гейт не может работать в GitHub Actions по двум независимым причинам.

1. **В Actions нет `polyforge-mcp`.** `gate.sh` поднимает команду из
   `POLYFORGE_CMD` (иначе из `modules.toml`), по умолчанию `polyforge-mcp`, по
   stdio и шлёт JSON-RPC `gate_evaluate`. В чистом runner'е этой команды нет;
   без неё гейт завершается exit 2 с `MissingPolyForge` (fail-closed), то есть
   workflow всегда красный. Установка команды в workflow добавила бы внешнюю
   зависимость в CI без отдельного решения.

2. **В Actions нет `task_id`.** `gate.sh:78-80` требует непустой первый
   аргумент и при его отсутствии завершается exit 2 с `MissingTaskId`.
   `github.head_ref` — имя ветки PR, а не идентификатор задачи PolyForge.
   Подменять одно другим значит превратить «нет ответа» в «есть решение»,
   что запрещено инвариантом 6 AGENTS.md: отсутствие данных — отдельное
   состояние, а не значение по умолчанию.

## Решение

1. **Workflow удалён.** Файл workflow `polyforge` удалён из репозитория.
   Контрактный тест `polyforge_workflow_is_removed` в
   `crates/stellwerk-cli/tests/ci_gate.rs` проверяет отсутствие файла; возврат
   файла роняет тест — RED/GREEN наблюдаемы.

2. **Гейт остаётся локальным и fail-closed.** `scripts/integrator/gate.sh` не
   меняется. Тесты 155-163 `ci_gate.rs` по-прежнему доказывают, что без
   PolyForge гейт возвращает exit 2 и `MissingPolyForge`, а не молча
   разрешает. Гейт вызывается локально с явным `task_id` и доступной командой
   `polyforge-mcp`; аттестация остаётся локальным действием с evidence.

3. **CI не выполняет гейт PolyForge.** Строгий tier-1 в `ci.yml` (ADR-0027)
   проверяет код, но не подменяет локальный гейт: в Actions нет ни команды,
   ни идентификатора задачи. Отсутствие гейта в CI — не дыра, а явная
   граница контракта, зафиксированная этим ADR.

## Последствия

- Возврат workflow `polyforge` без `polyforge-mcp` и настоящего `task_id` —
  дефект, который ловит контрактный тест.
- Локальный гейт — единственное место, где формируется ToolAttestation для
  PolyForge; `just check` и CI не выдают аттестацию вместо него.
- Тест fail-closed без PolyForge (155-163) сохраняется как отдельное
  доказательство того, что отсутствие команды не трактуется как успех.
- Любая попытка перенести гейт в Actions — `contract-change` с новым ADR,
  который должен явно решить, откуда берутся команда и `task_id`.

## Что запрещено

1. Возвращать workflow, вызывающий `gate.sh`, в `.github/workflows/`.
2. Подменять `task_id` именем ветки, SHA или другим значением по умолчанию.
3. Делать гейт обязательным check в branch protection без доступных
   `polyforge-mcp` и `task_id`.
4. Менять `gate.sh` так, чтобы отсутствие команды или `task_id` перестало быть
   fail-closed.
5. Удалять или ослаблять тест fail-closed без PolyForge (155-163).

## Когда пересматривается

Если в GitHub Actions появится действие, предоставляющее `polyforge-mcp` и
явное сопоставление события с `task_id`, этот ADR пересматривается отдельным
пакетом с меткой `contract-change`. Пересмотр обязан сохранить различие между
«команда и задача есть» и «никто не ответил» — в статусе и в evidence.
