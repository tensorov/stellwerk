#!/usr/bin/env bash
# w2_preflight.sh — аналог base_preflight, но для W2.
#
# Проверяет на чистом BASE `2d572f1`:
#   * наличие файлов, на которые ссылается packet-07a (иначе агент уйдёт искать их наружу);
#   * `cargo test -p stellwerk-registry measure` → ожидаемый RED (пакет-07a делает measure GREEN).
# Фикстура НЕ нужна: BASE содержит cmd_measure.rs (221 строка) и lib.rs объявляет модуль.
set -uo pipefail
umask 077
REPO="${STELLWERK_REPO:-/home/izyabretatel/gits/stellwerk}"
BASE="2d572f1"
OUT="$REPO/.ablation-solpi/runs/.preflight"
FILEREF="$REPO/.worktrees/m0/measure/crates/stellwerk-registry/src/cmd_measure.rs"

mkdir -p "$OUT"
TMP=$(mktemp -d /tmp/opencode/w2pf.XXXXXX); WT="$TMP/wt"
export CARGO_TARGET_DIR="$TMP/target"
export CI=true DEBIAN_FRONTEND=noninteractive GIT_TERMINAL_PROMPT=0
export GIT_EDITOR=: EDITOR=: VISUAL='' GIT_SEQUENCE_EDITOR=: GIT_MERGE_AUTOEDIT=no
export GIT_PAGER=cat PAGER=cat

echo "== создаю detached worktree на $BASE =="
git -C "$REPO" worktree add --detach "$WT" "$BASE" >/dev/null 2>&1 || { echo "FAIL: worktree"; exit 1; }
echo "HEAD в worktree: $(git -C "$WT" rev-parse HEAD)"

echo "== наличие файлов, которые упоминает packet-07a =="
MISSING=""
for rel in crates/stellwerk-registry/src/cmd_measure.rs \
           crates/stellwerk-registry/src/cli.rs \
           crates/stellwerk-registry/src/lib.rs \
           crates/stellwerk-registry/src/model.rs; do
  if [ -f "$WT/$rel" ]; then
    printf '  OK      %-52s %s строк\n' "$rel" "$(wc -l < "$WT/$rel")"
  else
    printf '  MISSING %s\n' "$rel"; MISSING="$MISSING $rel"
  fi
done
echo "lib.rs объявляет модуль: $(grep -c 'pub mod cmd_measure' "$WT/crates/stellwerk-registry/src/lib.rs")"

echo "== cargo test -p stellwerk-registry measure =="
( cd "$WT" && cargo test -p stellwerk-registry measure ) > "$OUT/w2_tests.txt" 2>&1
RC=$?
echo "exit code: $RC"
grep -E "^test result:" "$OUT/w2_tests.txt" | head -3
echo "первые упавшие тесты:"
grep -E "^test .* FAILED|panicked at" "$OUT/w2_tests.txt" | head -5

git -C "$REPO" worktree remove --force "$WT"
rm -rf "$TMP"
echo "worktree удалён: $([ -e "$WT" ] && echo НЕТ || echo ДА)"

python3 - "$OUT/w2_preflight.json" "$RC" "$MISSING" <<'PY'
import hashlib, json, os, subprocess, sys
out, rc, missing = sys.argv[1], int(sys.argv[2]), sys.argv[3].split()
repo = os.environ.get('STELLWERK_REPO', '/home/izyabretatel/gits/stellwerk')
def sha(p):
    try:
        return hashlib.sha256(open(p, 'rb').read()).hexdigest()
    except OSError:
        return None
base_cmd = ['git', '-C', repo, 'show', '2d572f1:crates/stellwerk-registry/src/cmd_measure.rs']
base_src = subprocess.run(base_cmd, capture_output=True).stdout
res = {
    'base': '2d572f1',
    'measure_test_exit': rc,
    'expected': 'RED (non-zero) — packet-07a делает measure GREEN',
    'observed_red': rc != 0,
    'missing_files': missing,
    'premise_ok': not missing,
    'cmd_measure_sha256_base': hashlib.sha256(base_src).hexdigest(),
    'cmd_measure_lines_base': base_src.count(b'\n'),
    'cmd_measure_sha256_m0_measure': sha(
        os.path.join(repo, '.worktrees/m0/measure/crates/stellwerk-registry/src/cmd_measure.rs')),
    'fixture_needed': False,
}
json.dump(res, open(out, 'w'), indent=1, ensure_ascii=False)
print(json.dumps(res, indent=1, ensure_ascii=False))
PY
