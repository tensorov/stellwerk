# stellwerk-federation

Control plane для агентной работы. Ядро скучное: events, RBAC, sync, evidence, registry.
Агенты сменные. Память разделена. Obsidian — окно. PolyForge — закон.

Текущий milestone: **M0** — см. `docs/M0.md`. Промпт для Prometheus — `docs/PROMETHEUS_PROMPT.md`.

    just doctor      # приёмка dev-станции
    just check       # fmt + clippy + test
    just plan-lint   # приёмка плана перед Momus/Metis
