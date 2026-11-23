#!/usr/bin/env bash
# accept_schema.sh — приёмка текущих форматов пакетов (N2 YAML, N1/TUI headings).
#
# Проверяет, что accept.sh читает:
#   - N2: frontmatter `allowed_changes`, секции `## RED` / `## Приёмка`;
#   - N1/TUI: `## files_allowed`, `## Приёмка`, отсутствие `## RED`;
#   - отсутствие `cargo test` — это `NO_TEST_COMMAND`, а не «принято».
#
# Запуск: bash scripts/integrator/tests/accept_schema.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TMP=$(mktemp -d /tmp/accept-schema-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
export STELLWERK_PLANS_DIR="$TMP/plans"
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); printf '  \033[32mok\033[0m   %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s\n' "$1"; }
mk()  { mkdir -p "$(dirname "$1")"; printf '%s\n' "$2" > "$1"; }

# Песочница-заглушка: один зелёный и один падающий тест. Реальные продуктовые
# тесты не запускаются — только эти временные фикстуры.
S="$TMP/sb"; mkdir -p "$S/tests"
mk "$S/Cargo.toml" '[package]
name = "schema_fixture"
version = "0.0.1"
edition = "2021"'
mk "$S/tests/ok.rs" '#[test]
fn t() { assert_eq!(2 + 2, 4); }'
mk "$S/tests/broken.rs" '#[test]
fn t() { assert_eq!(1, 2); }'

accept() { "$HERE/../accept.sh" "$@" 2>&1; }

# ── N2 RED: allowed_changes + ## RED ────────────────────────────────────────
echo "N2 RED: allowed_changes и команда из ## RED"
mk "$TMP/plans/N2/packet-red.md" '---
id: N2/red
milestone: N2
title: red fixture
depends_on: []
allowed_changes:
  - crates/x/src/a.rs
  - crates/x/tests/a.rs
---

# red fixture

## RED

```
cargo test --test broken
```

## Приёмка

```
cargo test --test ok
```
'
out="$(accept N2 packet-red "$S")"; rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q 'RED-пакет' \
   && printf '%s' "$out" | grep -q 'crates/x/src/a.rs'; then
  ok "N2 RED принят по падению из ## RED, allowed_changes прочитан"
else
  bad "N2 RED не принят или allowed_changes не прочитан (rc=$rc)"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── N2 GREEN: allowed_changes + ## Приёмка ──────────────────────────────────
echo "N2 GREEN: allowed_changes и команда из ## Приёмка"
mk "$TMP/plans/N2/packet-green.md" '---
id: N2/green
milestone: N2
title: green fixture
depends_on: []
allowed_changes:
  - crates/x/src/a.rs
---

# green fixture

## Приёмка

```
cargo test --test ok
```
'
out="$(accept N2 packet-green "$S")"; rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q 'GREEN-пакет' \
   && printf '%s' "$out" | grep -q 'crates/x/src/a.rs'; then
  ok "N2 GREEN принят по ## Приёмка, allowed_changes прочитан"
else
  bad "N2 GREEN не принят или allowed_changes не прочитан (rc=$rc)"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── N1: ## files_allowed + ## Приёмка ───────────────────────────────────────
echo "N1: ## files_allowed и ## Приёмка"
mk "$TMP/plans/N1/packet-01.md" '# N1-01 — fixture

**depends_on:** []

## files_allowed

- `crates/a/src/main.rs`
- `crates/a/tests/run.rs`

## Приёмка

```sh
cargo test --test ok
```
'
out="$(accept N1 packet-01 "$S")"; rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q 'GREEN-пакет' \
   && printf '%s' "$out" | grep -q 'crates/a/src/main.rs'; then
  ok "N1 принят как GREEN, files_allowed прочитан"
else
  bad "N1 не принят или files_allowed не прочитан (rc=$rc)"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── TUI: нет cargo test → NO_TEST_COMMAND ───────────────────────────────────
echo "TUI: нет cargo test — NO_TEST_COMMAND, не принято"
mk "$TMP/plans/TUI/packet-01.md" '# TUI-01 — fixture

**depends_on:** [TUI-00]

## files_allowed

- `Cargo.toml`
- `crates/stellwerk-cli/Cargo.toml`

## Приёмка

```sh
ssh example.invalid "cd ~/gits/stellwerk && cargo build --workspace --all-targets"
```
'
out="$(accept TUI packet-01 "$S")"; rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'NO_TEST_COMMAND'; then
  ok "TUI отклонён с NO_TEST_COMMAND"
else
  bad "TUI не дал NO_TEST_COMMAND или вернул 0 (rc=$rc)"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── пакет без команды вообще → NO_TEST_COMMAND ──────────────────────────────
echo "N2 без команды: NO_TEST_COMMAND, не принято"
mk "$TMP/plans/N2/packet-nocmd.md" '---
id: N2/nocmd
milestone: N2
title: no command
depends_on: []
allowed_changes:
  - crates/x/src/a.rs
---

# no command

## Приёмка

Ожидание: тесты зелёные.
'
out="$(accept N2 packet-nocmd "$S")"; rc=$?
if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'NO_TEST_COMMAND'; then
  ok "пакет без cargo test отклонён с NO_TEST_COMMAND"
else
  bad "пакет без cargo test не отклонён (rc=$rc)"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── RED с зелёным тестом обязан быть отклонён ───────────────────────────────
echo "N2 RED с зелёным тестом: отклонено"
mk "$TMP/plans/N2/packet-red-green.md" '---
id: N2/red-green
milestone: N2
title: red but green
depends_on: []
allowed_changes:
  - crates/x/src/a.rs
---

# red but green

## RED

```
cargo test --test ok
```
'
out="$(accept N2 packet-red-green "$S")"; rc=$?
if [ "$rc" -ne 0 ]; then
  ok "RED-пакет с зелёным тестом отклонён"
else
  bad "RED-пакет с зелёным тестом принят — инверсия не работает"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

# ── GREEN без строки 'test result: ok.' обязан быть отклонён ────────────────
echo "N2 GREEN без 'test result: ok.': отклонено"
mk "$TMP/plans/N2/packet-green-noresult.md" '---
id: N2/green-noresult
milestone: N2
title: green without result line
depends_on: []
allowed_changes:
  - crates/x/src/a.rs
---

# green without result line

## Приёмка

```
cargo test --test ok -- --list
```
'
out="$(accept N2 packet-green-noresult "$S")"; rc=$?
if [ "$rc" -ne 0 ]; then
  ok "GREEN-пакет без 'test result: ok.' отклонён"
else
  bad "GREEN-пакет без 'test result: ok.' принят — приёмка пустая"
  printf '%s\n' "$out" | sed 's/^/        /'
fi

printf '\n────────────────────────────────────────\n'
printf 'passed: %d   failed: %d\n' "$PASS" "$FAIL"
[ "$FAIL" -gt 0 ] && exit 1
printf 'ACCEPT_SCHEMA_OK\n'
exit 0
