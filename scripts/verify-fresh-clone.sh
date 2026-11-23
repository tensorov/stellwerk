#!/usr/bin/env bash
# Fresh-clone proof with two explicit tiers.
#
# tier-1 (always): build the CLI from a clean clone, smoke-test --help, run the
#                  strict gate (which includes the history scan), scan history
#                  again explicitly, run the xtask ledger test, and assert that
#                  no evidence file is tracked in the public tree.
# tier-2 (conditional): full workspace tests, only when every prerequisite is
#                       present. A missing prerequisite is a named SKIP with
#                       exit 2 — never a silent success.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

# --no-local forces a real git transport clone with full history; a shallow
# clone would make scan-history.sh fail closed and defeat the proof.
printf '[fresh-clone] cloning %s into %s (--no-local, full history)\n' "$REPO" "$TMP"
git clone --no-local "$REPO" "$TMP"
cd "$TMP"

# ── tier-1: always runs ──────────────────────────────────────────────────────
printf '[tier-1] cargo build --locked -p stellwerk-cli --bin stellwerk\n'
cargo build --locked -p stellwerk-cli --bin stellwerk

printf '[tier-1] ./target/debug/stellwerk --help\n'
./target/debug/stellwerk --help

printf '[tier-1] just check (includes history)\n'
just check

printf '[tier-1] bash scripts/scan-history.sh\n'
bash scripts/scan-history.sh

printf '[tier-1] cargo test -p xtask --test ledger_encoding\n'
cargo test -p xtask --test ledger_encoding

evidence_count="$(git ls-files .omo/evidence | wc -l)"
evidence_count="${evidence_count//[[:space:]]/}"
if [ "$evidence_count" != "0" ]; then
    printf '[tier-1] FAIL: %s tracked file(s) under .omo/evidence in the fresh clone\n' \
        "$evidence_count" >&2
    exit 1
fi
printf '[tier-1] tracked .omo/evidence files: %s\n' "$evidence_count"

printf 'FRESH CLONE: OK (tier-1)\n'

# ── tier-2: only when every prerequisite is present ──────────────────────────
missing=()

if ! command -v rustup >/dev/null 2>&1; then
    missing+=("rustup")
elif ! toolchains="$(rustup toolchain list 2>/dev/null)" || [[ "$toolchains" != *nightly-2026-10-01* ]]; then
    missing+=("nightly-2026-10-01")
fi

command -v python3 >/dev/null 2>&1 || missing+=("python3")
command -v bwrap >/dev/null 2>&1 || missing+=("bwrap")
[ -n "${PI_BIN-}" ] || missing+=("PI_BIN")
[ -n "${STELLWERK_PI_EXTENSION-}" ] || missing+=("STELLWERK_PI_EXTENSION")
[ -n "${NEURALDEEP_TOKEN-}" ] || missing+=("NEURALDEEP_TOKEN")

if [ "${#missing[@]}" -gt 0 ]; then
    missing_list="$(IFS=,; printf '%s' "${missing[*]}")"
    printf 'TIER-2 SKIPPED: %s\n' "$missing_list"
    exit 2
fi

printf '[tier-2] cargo test --workspace --exclude xtask --locked\n'
cargo test --workspace --exclude xtask --locked
printf 'FRESH CLONE: OK (tier-1+tier-2)\n'
