СКОУП ЭТОГО ПЛАНА: только M0 = crate stellwerk-registry (module add|measure|audit|update).
Решения по M0 уже приняты и записаны в docs/M0.md — не переспрашивай их.
Всё в docs/DEFERRED.md и любые упоминания kanban/hermes/gateway/nag/crm/obsidian/sync —
контекст будущей системы, планировать запрещено. Вопрос, влияющий только на M1+, не задавай —
запиши в docs/DEFERRED.md.

Состояние репозитория:
- crates/ — пустой скелет, кода нет.
- scripts/ablation/, docs/experiments/, docs/CONTEXT_BUDGET.md — законченный измерительный
  инструмент. Не трогать, не улучшать, не планировать. Можно читать как источник чисел.

МОДЕЛЬ ИСПОЛНЕНИЯ (измерено, см. docs/CONTEXT_BUDGET.md):
- Исполнитель — НЕ Sisyphus и НЕ OMO-агент. Это clean OpenCode agent `stellwerk-micro`
  на qwen, лимит 46 112 токенов, baseline ≈9.4k, рабочий бюджет ≤36k.
- Не планируй /start-work и делегирование через Atlas/Sisyphus. Результат плана —
  набор файлов .sisyphus/plans/M0/packet-NN.md, которые я раздам воркерам сам.
- Воркер видит ТОЛЬКО свой packet + перечисленные файлы. Полный план он не читает.
  Packet должен быть самодостаточен.

Формат каждого packet (строго):
  # packet-NN
  plan_id: M0
  goal: <одно предложение>
  files_allowed: [<≤3 пути>]
  max_diff_lines: 200
  test: cargo test -p stellwerk-registry <точный фильтр>
  acceptance: <проверяемый предикат>
  forbidden: [cargo add, правка корневого Cargo.toml, правка modules.lock]
  depends_on: [<packet-ids>]
  packet ≤ 4k токенов.

Ограничения, не подлежащие обсуждению:
- TDD: первый packet каждой команды — падающий тест; PolyForge — единственный источник VERIFIED;
- зависимости только из [workspace.dependencies] корневого Cargo.toml; новые — BLOCKED;
- всего packets ≤ 12; независимые packets помечай для параллельного запуска (до 3 одновременно).

Калибровочная точка для `measure` (измерено): схемы 4 тулов polyforge-mcp стоят
813 токенов на qwen (1 063 на big-pickle). Acceptance для `measure`: на polyforge-mcp
выдаёт 813 ± 10% и записывает это в modules.lock.

Проинтервьюируй меня. Затем план и packets.

ВОРКЕР-РАНТАЙМ: Pi v0.87.0, команда stellwerk-worker-pi.
Overhead ~2 426 токенов. Рабочее окно ~43 686 токенов.
PolyForge: polyforge-cli через bash-тул, не MCP.
Воркер не видит план — только packet-NN.md.
