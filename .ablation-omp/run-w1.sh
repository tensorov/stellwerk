#!/usr/bin/env bash
# 2.2: W1 на OMP, n=3, каждая реп в свежей песочнице без .git
set -u
ROOT=/home/izyabretatel/gits/stellwerk
PI="$ROOT/.ablation-omp/omp-install/node_modules/.bin/pi"
OMP_EXT="$ROOT/.ablation-omp/omp-install/node_modules/oh-my-pi/dist/extension.js"
mkdir -p "$ROOT/.ablation-omp/runs"
for r in 1 2 3; do
  D=$(mktemp -d /tmp/omp-w1-run${r}-XXXXXX)
  git -C "$ROOT" archive 2d572f1 | tar -x -C "$D"
  printf '{\n  "orchestrator": { "agentName": "oh-my-pi", "promptTemplate": "sisyphus" }\n}\n' > "$D/.oh-my-pi.jsonc"
  OUT="$ROOT/.ablation-omp/runs/w1-omp-${r}.jsonl"
  ( cd "$D" && exec env PI_CODING_AGENT_SESSION_DIR="$D/.sess" timeout 600 "$PI" \
      --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off \
      -p "$(cat "$ROOT/.ablation-omp/w1-packet.md")" --mode json --no-prompt-templates \
      --no-session --no-context-files --no-skills --no-themes \
      -e "$OMP_EXT" ) > "$OUT" 2>"$OUT.stderr"
  RC=$?
  TEST=$( cd "$D" && timeout 600 cargo test -p stellwerk-registry model 2>&1 | grep -oE "w1_(alpha|beta|gamma)" | sort -u | tr '\n' ',' )
  echo "rep$r: pi_exit=$RC песочница=$D тесты=[$TEST]"
  echo "$D" > "$ROOT/.ablation-omp/w1-done-$r.txt"
done
