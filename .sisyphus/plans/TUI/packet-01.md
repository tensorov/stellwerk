# TUI-01 — контракт-зависимость: TUI-крейты и MSRV

**label:** contract-change · **mutation_gate:** no (манифесты и ADR, кода нет)
**depends_on:** [TUI-00] · **ADR:** `docs/adr/0018-tui-dependencies.md` — обязателен
overlap_waiver: C9 — N1/packet-01 и TUI/01 делят crates/stellwerk-cli/Cargo.toml, последовательное исполнение

## Цель

Ввести `ratatui` и `crossterm` в `[workspace.dependencies]` и поднять
`rust-version`. Обоснование выбора и версий фиксируется ADR — закон 5
(зависимости) не даёт выбирать их молча.

Решение владельца: `ratatui` + `crossterm`; `rust-version` поднимается,
потому что актуальные `ratatui` не влезают в `1.80` (`Cargo.toml:8`).

## production_call

`stellwerk-cli` получает доступ к рендеру через `workspace = true`.

## files_allowed

- `Cargo.toml`
- `crates/stellwerk-cli/Cargo.toml`
- `Cargo.lock`
- `docs/adr/0018-tui-dependencies.md`

**limit_waiver:** `files: 4` — `Cargo.lock` переписывается добавлением
зависимостей и не может не попасть в пакет; ADR обязателен по закону;
без `Cargo.toml` крейта нет.

## Что делать

1. В `Cargo.toml` в `[workspace.dependencies]` добавить **ровно две** записи
   с закреплёнными версиями (точный pin, не `*` и не диапазон), и поднять
   `rust-version` до значения, требуемого выбранной версией `ratatui`.

2. В `crates/stellwerk-cli/Cargo.toml` подключить через `workspace = true`.

3. Создать `docs/adr/0018-tui-dependencies.md` по образцу
   `docs/adr/0017-risk-classification-authority.md`: шапка
   `Статус / Дата / Пакет / Связано`, разделы `Проблема` / `Ключевой факт` /
   `Решение` / `Что запрещено` / `Когда пересматривается`. В ADR записать
   **фактические выбранные версии** и причину подъёма MSRV.

4. Обновить `Cargo.lock` — произойдёт при первой сборке на хосте.

**Дискриминирующая проверка пакета** (отличает «добавлено» от «не добавлено»):

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo tree -p stellwerk-cli | grep -c "ratatui v"; echo "RATATUI_EXIT=$?"'
```

`RATATUI_EXIT=0` и счётчик ≥ 1. Сборка сама по себе критерием не является —
`cargo build` проходит и без зависимостей.

## Запрещено трогать

`modules.toml`, `crates/stellwerk-core/**`, `crates/stellwerk-gateway/**`,
`crates/stellwerk-store/**`, `crates/stellwerk-registry/**`, `justfile`,
`.github/`, `scripts/plan-lint.py`, `xtask/**`, `mutation-evidence/`.
`cargo add` запрещён — правка манифестов ручная.
Любые крейты кроме `ratatui` и `crossterm`.

## Почему `mutation_gate: no`

Манифесты и ADR. Ни исполняемого кода, ни ветвлений — мутировать нечего.

## Приёмка

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo build --workspace --all-targets'; echo "BUILD_EXIT=$?"
```

`BUILD_EXIT=0`.

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && cargo tree -p stellwerk-cli | grep -c "ratatui v"'; echo "RESOLVES=$?"
```

Обязан напечатать `1` (или больше) и `RESOLVES=0`.

```sh
ssh 192.168.1.11 'cd ~/gits/stellwerk && grep -nE "^(ratatui|crossterm)[[:space:]]*=" Cargo.toml'; echo "MANIFEST_EXIT=$?"
```

Ровно две строки. ADR обязан содержать `Пакет: TUI-01` и совпадать с
фактическими версиями из `Cargo.toml` (сверяется в F5).