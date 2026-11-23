#!/usr/bin/env bash
# Test for scripts/doctor.sh — fail-closed prerequisite checks.
# Runs from the repository root so the .sisyphus/plans and opencode.json
# checks see the real checkout while every external tool is a stub.
set -u

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

DOCTOR="$REPO_ROOT/scripts/doctor.sh"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

pass() {
    printf 'ok - %s\n' "$*"
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

STUBS="$TMP/stubs"
mkdir -p "$STUBS" "$TMP/home"

# An executable stub that prints a plausible version line and exits 0.
make_stub() {
    stub_path="$STUBS/$1"
    printf '#!/bin/sh\nprintf "%%s 1.2.3\\n" "%s"\nexit 0\n' "$1" > "$stub_path"
    chmod +x "$stub_path"
}

for tool in cargo rustc opencode polyforge-mcp gh bwrap pi; do
    make_stub "$tool"
done

# Temp extension file and temp token file under a temp XDG_CONFIG_HOME.
EXT="$TMP/stellwerk-extension.mjs"
printf '// stub extension\n' > "$EXT"

XDG="$TMP/config"
mkdir -p "$XDG/stellwerk"
TOKEN="$XDG/stellwerk/neuraldeep.token"
printf 'stub-token-value\n' > "$TOKEN"

# Minimal bin dir with only the externals doctor.sh needs (sh + grep),
# used to prove a missing `gh` is soft without a system gh leaking in.
MIN_BIN="$TMP/min-bin"
mkdir -p "$MIN_BIN"
ln -s "$(command -v sh)" "$MIN_BIN/sh"
ln -s "$(command -v grep)" "$MIN_BIN/grep"

STUBS_NO_GH="$TMP/stubs-no-gh"
mkdir -p "$STUBS_NO_GH"
for tool in cargo rustc opencode polyforge-mcp bwrap pi; do
    cp "$STUBS/$tool" "$STUBS_NO_GH/$tool"
done

# ---------------------------------------------------------------------------
# 1. Happy path: every mandatory check green, gh present.
# ---------------------------------------------------------------------------
out="$(
    export PATH="$STUBS:/usr/bin:/bin"
    export HOME="$TMP/home"
    export PI_BIN="$STUBS/pi"
    export STELLWERK_PI_EXTENSION="$EXT"
    export XDG_CONFIG_HOME="$XDG"
    sh "$DOCTOR" 2>&1
)"
rc=$?
[ "$rc" -eq 0 ] || fail "happy: expected exit 0, got $rc; output: $out"
for line in \
    'cargo: OK' \
    'rustc: OK' \
    'opencode: OK' \
    'polyforge-mcp: OK' \
    'gh: OK' \
    'pi: OK' \
    'STELLWERK_PI_EXTENSION: OK' \
    'token: OK' \
    'bwrap: OK' \
    '.sisyphus/plans: ok' \
    'subagent_depth=1: ok'; do
    printf '%s\n' "$out" | grep -qF "$line" || fail "happy: missing '$line' in: $out"
done
pass "happy path exits 0 with all OK lines"

# ---------------------------------------------------------------------------
# 2. PI_BIN=/nonexistent -> exit 1 and 'pi: MISSING'.
# ---------------------------------------------------------------------------
out="$(
    export PATH="$STUBS:/usr/bin:/bin"
    export HOME="$TMP/home"
    export PI_BIN=/nonexistent
    export STELLWERK_PI_EXTENSION="$EXT"
    export XDG_CONFIG_HOME="$XDG"
    sh "$DOCTOR" 2>&1
)"
rc=$?
[ "$rc" -eq 1 ] || fail "PI_BIN=/nonexistent: expected exit 1, got $rc; output: $out"
printf '%s\n' "$out" | grep -qF 'pi: MISSING' || fail "PI_BIN=/nonexistent: missing 'pi: MISSING' in: $out"
pass "PI_BIN=/nonexistent fails closed with pi: MISSING"

# ---------------------------------------------------------------------------
# 3. STELLWERK_PI_EXTENSION unset -> exit 1 and UNCONFIGURED line.
# ---------------------------------------------------------------------------
out="$(
    export PATH="$STUBS:/usr/bin:/bin"
    export HOME="$TMP/home"
    export PI_BIN="$STUBS/pi"
    unset STELLWERK_PI_EXTENSION
    export XDG_CONFIG_HOME="$XDG"
    sh "$DOCTOR" 2>&1
)"
rc=$?
[ "$rc" -eq 1 ] || fail "unset extension: expected exit 1, got $rc; output: $out"
printf '%s\n' "$out" | grep -qF 'STELLWERK_PI_EXTENSION: UNCONFIGURED' \
    || fail "unset extension: missing UNCONFIGURED line in: $out"
pass "unset extension fails closed with STELLWERK_PI_EXTENSION: UNCONFIGURED"

# ---------------------------------------------------------------------------
# 4. gh missing is soft: exit 0 while every mandatory check passes.
# ---------------------------------------------------------------------------
out="$(
    export PATH="$STUBS_NO_GH:$MIN_BIN"
    export HOME="$TMP/home"
    export PI_BIN="$STUBS/pi"
    export STELLWERK_PI_EXTENSION="$EXT"
    export XDG_CONFIG_HOME="$XDG"
    sh "$DOCTOR" 2>&1
)"
rc=$?
[ "$rc" -eq 0 ] || fail "soft gh: expected exit 0, got $rc; output: $out"
printf '%s\n' "$out" | grep -qF 'gh: MISSING' || fail "soft gh: missing 'gh: MISSING' in: $out"
pass "missing gh is soft (exit 0) with gh: MISSING reported"

# ---------------------------------------------------------------------------
# 5. Empty token file fails closed (mode 0600 is deliberately NOT checked).
# ---------------------------------------------------------------------------
: > "$TOKEN"
out="$(
    export PATH="$STUBS:/usr/bin:/bin"
    export HOME="$TMP/home"
    export PI_BIN="$STUBS/pi"
    export STELLWERK_PI_EXTENSION="$EXT"
    export XDG_CONFIG_HOME="$XDG"
    sh "$DOCTOR" 2>&1
)"
rc=$?
[ "$rc" -eq 1 ] || fail "empty token: expected exit 1, got $rc; output: $out"
printf '%s\n' "$out" | grep -qF 'token: MISSING' || fail "empty token: missing 'token: MISSING' in: $out"
pass "empty token fails closed with token: MISSING"

printf 'ALL DOCTOR TESTS PASSED\n'
