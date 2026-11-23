#!/usr/bin/env bash
# Test for scripts/scan-history.sh — all-history secret scan, fail-closed.
# Each case uses its own throwaway repository so no fixture leaks into the real tree.
set -u

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCANNER="$REPO_ROOT/scripts/scan-history.sh"
SHELL_BIN="$(command -v sh)"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

pass() {
    printf 'ok - %s\n' "$*"
}

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

# Assembled at runtime: a literal realistic token in this committed test file
# would itself be found by the scanner when it walks the real repository history.
TOKEN_PREFIX='ghp_'
TOKEN_BODY='AAAAAAAAAAAAAAAAAAAAAAAA'
FAKE_TOKEN="${TOKEN_PREFIX}${TOKEN_BODY}"

make_repo() {
    dir="$1"
    mkdir -p "$dir"
    git -C "$dir" init -q
    git -C "$dir" -c user.name=test -c user.email=test@example.invalid \
        commit -q --allow-empty -m init
}

commit_file() {
    dir="$1"
    name="$2"
    git -C "$dir" add "$name"
    git -C "$dir" -c user.name=test -c user.email=test@example.invalid \
        commit -q -m "add $name"
}

# ---------------------------------------------------------------------------
# 1. Fake token in a commit -> exit 1, only commit:path printed, value hidden.
# ---------------------------------------------------------------------------
REPO_TOKEN="$TMP/with-token"
make_repo "$REPO_TOKEN"
printf 'credential: %s\n' "$FAKE_TOKEN" > "$REPO_TOKEN/leak.txt"
commit_file "$REPO_TOKEN" leak.txt

out="$(cd "$REPO_TOKEN" && "$SHELL_BIN" "$SCANNER" 2>&1)"
rc=$?
[ "$rc" -eq 1 ] || fail "fake token: expected exit 1, got $rc; output: $out"
printf '%s\n' "$out" | grep -qF "$FAKE_TOKEN" \
    && fail "fake token: secret value leaked into output: $out"
printf '%s\n' "$out" | grep -qE '^[0-9a-f]+:leak\.txt$' \
    || fail "fake token: expected commit:path output, got: $out"
pass "fake token -> exit 1, commit:path only, value not printed"

# ---------------------------------------------------------------------------
# 2. Clean repository -> exit 0.
# ---------------------------------------------------------------------------
REPO_CLEAN="$TMP/clean"
make_repo "$REPO_CLEAN"
printf 'hello world\n' > "$REPO_CLEAN/README.md"
commit_file "$REPO_CLEAN" README.md

out="$(cd "$REPO_CLEAN" && "$SHELL_BIN" "$SCANNER" 2>&1)"
rc=$?
[ "$rc" -eq 0 ] || fail "clean: expected exit 0, got $rc; output: $out"
pass "clean repository -> exit 0"

# ---------------------------------------------------------------------------
# 3. Shallow repository -> exit 2 (scan not possible, never 'clean').
# ---------------------------------------------------------------------------
REPO_SHALLOW="$TMP/shallow"
make_repo "$REPO_SHALLOW"
printf '%s\n' "$(git -C "$REPO_SHALLOW" rev-parse HEAD)" > "$REPO_SHALLOW/.git/shallow"
[ "$(git -C "$REPO_SHALLOW" rev-parse --is-shallow-repository)" = true ] \
    || fail "shallow fixture: git did not recognise the shallow marker"

out="$(cd "$REPO_SHALLOW" && "$SHELL_BIN" "$SCANNER" 2>&1)"
rc=$?
[ "$rc" -eq 2 ] || fail "shallow: expected exit 2, got $rc; output: $out"
pass "shallow repository -> exit 2"

# ---------------------------------------------------------------------------
# 4. Missing git -> exit 2 (fail closed, never 'clean').
# ---------------------------------------------------------------------------
EMPTY_BIN="$TMP/empty-bin"
mkdir -p "$EMPTY_BIN"
out="$(cd "$REPO_CLEAN" && PATH="$EMPTY_BIN" "$SHELL_BIN" "$SCANNER" 2>&1)"
rc=$?
[ "$rc" -eq 2 ] || fail "missing git: expected exit 2, got $rc; output: $out"
pass "missing git -> exit 2"

printf 'ALL SCAN-HISTORY TESTS PASSED\n'
