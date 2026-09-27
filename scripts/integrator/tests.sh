#!/usr/bin/env bash
# tests.sh — тесты на три реальных промаха харнесса приёмки (сессия #2).
# Запуск: scripts/integrator/tests.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
TMP=$(mktemp -d /tmp/accept-tests-XXXXXX)
trap 'rm -rf "$TMP"' EXIT
export STELLWERK_PLANS_DIR="$TMP/plans"
PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); printf '  \033[32mok\033[0m   %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf '  \033[31mFAIL\033[0m %s\n' "$1"; }
mk()  { mkdir -p "$(dirname "$1")"; printf '%s\n' "$2" > "$1"; }

# Песочница-заглушка: Cargo.toml нулен, один зелёный тест.
S="$TMP/sb"; mkdir -p "$S/tests"
mk "$S/Cargo.toml" '[package]
name = "x"
version = "0.0.1"
edition = "2021"'
mk "$S/tests/ok.rs" '#[test]
fn t() { assert_eq!(2+2, 4); }'
mk "$S/tests/broken.rs" '#[test]
fn t() { assert_eq!(1, 2); }'

# ── промах 1: milestone зашит ───────────────────────────────────────────────
# Проверяем, что пакет из ДРУГОГО milestone не берётся по неверному пути:
# accept.sh обязан читать test: из каталога переданного milestone.
echo "промах 1: milestone задаётся параметром, а не константой"
M1="$TMP/plans/M1"; M2="$TMP/plans/M2"
PL="$TMP/plans/M1"; PL2="$TMP/plans/M2"
mkdir -p "$M1" "$M2"
mk "$PL/packet-01.md" '# packet-01
test: cargo test --test definitely_missing_target
acceptance: -> exit 0'
mk "$PL2/packet-01.md" '# packet-01
test: cargo test --test ok
acceptance: -> exit 0'
if "$HERE/accept.sh" M2 packet-01 "$S" >/dev/null 2>&1; then
  ok "приёмка пакета M2 проходит со своим test:"
else
  bad "приёмка пакета M2 не прошла"
fi
# С тем же sandbox, но пакетом из M1 (несуществующий target) — обязан упасть.
if "$HERE/accept.sh" M1 packet-01 "$S" >/dev/null 2>&1; then
  bad "пакет M1 с несуществующим target прошёл — milestone подменён?"
else
  ok "чужой milestone даёт отказ, а не чужой результат"
fi

# ── промах 2: один критерий на RED и GREEN ───────────────────────────────────
echo "промах 2: RED и GREEN имеют разные критерии"
mk "$PL/packet-02.md" '# packet-02
goal: Реализовать предикат
test: cargo test --test ok
acceptance: -> 1 passed; 0 failed'
if "$HERE/accept.sh" M1 packet-02 "$S" >/dev/null 2>&1; then
  ok "GREEN-пакет с зелёным тестом принят"
else
  bad "GREEN-пакет отклонён"
fi
mk "$PL/packet-03.md" '# packet-03
goal: RED-пакет, спецификация
test: cargo test --test broken
acceptance: тест должен упасть (ожидается падение)'
if "$HERE/accept.sh" M1 packet-03 "$S" >/dev/null 2>&1; then
  ok "RED-пакет с падающим тестом принят по инвертированному критерию"
else
  bad "RED-пакет отклонён: критерий не инвертирован"
fi
# RED-пакет с ЗЕЛЁНЫМ тестом обязан быть отклонён (приёмка не инвертирована).
mk "$PL/packet-04.md" '# packet-04
goal: RED-пакет, спецификация
test: cargo test --test ok
acceptance: -> exit 0'
if "$HERE/accept.sh" M1 packet-04 "$S" >/dev/null 2>&1; then
  bad "RED-пакет с зелёным тестом принят — критерий не инвертирован"
else
  ok "RED-пакет с зелёным тестом отклонён (инверсия работает)"
fi

# ── промах 3: лимит файлов не проверялся ────────────────────────────────────
echo "промах 3: лимит файлов проверяется, limit_waiver учитывается"
MANY="$TMP/sb2"; mkdir -p "$MANY"
mk "$MANY/Cargo.toml" '[package]
name = "y"
version = "0.0.1"
edition = "2021"'
for i in 1 2 3 4 5; do mk "$MANY/f$i.rs" "// file $i"; done
mk "$MANY/tests/ok.rs" '#[test]
fn t() { assert_eq!(2+2, 4); }' 
mkdir -p "$MANY.run"
{ echo '--- a/f1.rs'; echo '+++ b/f1.rs'; echo '--- a/f2.rs'; echo '+++ b/f2.rs';
  echo '--- a/f3.rs'; echo '+++ b/f3.rs'; echo '--- a/f4.rs'; echo '+++ b/f4.rs';
  echo '--- a/f5.rs'; echo '+++ b/f5.rs'; } > "$MANY.run/sandbox.diff"
echo "files_changed=0 5 insertions=1 deletions=0" > "$MANY.run/sandbox.numstat"
mk "$PL/packet-05.md" '# packet-05
test: cargo test --test ok
files_allowed: [a.rs]
acceptance: -> exit 0'
if "$HERE/accept.sh" M1 packet-05 "$MANY" >/dev/null 2>&1; then
  bad "5 файлов без waiver принято — лимит не проверяется"
else
  ok "5 файлов без limit_waiver отклонено"
fi
mk "$PL/packet-06.md" '# packet-06
test: cargo test --test ok
files_allowed: [a.rs, b.rs, c.rs, d.rs, e.rs]
limit_waiver: include_str: миграция и регистрация — одна единица
acceptance: -> exit 0'
if "$HERE/accept.sh" M1 packet-06 "$MANY" >/dev/null 2>&1; then
  ok "5 файлов с limit_waiver принято"
else
  bad "5 файлов с limit_waiver отклонено — waiver не учитывается"
fi

printf '\n────────────────────────────────────────\n'
printf 'passed: %d   failed: %d\n' "$PASS" "$FAIL"
[ "$FAIL" -gt 0 ] && exit 1
exit 0
