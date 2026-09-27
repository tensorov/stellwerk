#!/usr/bin/env bash
# Overhead A/B: OMP-слой против vanilla Pi. Изолированные сессии внутри .ablation-omp.
set -u
ROOT=/home/izyabretatel/gits/stellwerk
PI="$ROOT/.ablation-omp/omp-install/node_modules/.bin/pi"
OMP_EXT="$ROOT/.ablation-omp/omp-install/node_modules/oh-my-pi/dist/extension.js"
SCRATCH="$ROOT/.ablation-omp/scratch-overhead"
mkdir -p "$SCRATCH"
printf '{\n  "orchestrator": { "agentName": "oh-my-pi", "promptTemplate": "sisyphus" }\n}\n' > "$SCRATCH/.oh-my-pi.jsonc"

run() { # $1=arm $2=rep
  local arm="$1" rep="$2"
  local out="$ROOT/.ablation-omp/runs/overhead-${arm}-${rep}.jsonl"
  mkdir -p "$(dirname "$out")"
  ( cd "$SCRATCH" && exec env \
      PI_CODING_AGENT_SESSION_DIR="$SCRATCH/session-${arm}" \
      timeout 240 "$PI" \
        --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off \
        -p "." --mode json --no-prompt-templates --no-session --no-context-files \
        --no-skills --no-themes \
        ${OMP:+-e "$OMP_EXT"} ) > "$out" 2>"$out.stderr"
  echo "  ${arm} rep${rep}: exit=$? bytes=$(wc -c < "$out")"
}

mkdir -p "$ROOT/.ablation-omp/runs"
for r in 1 2 3; do OMP=1 run omp "$r"; done
for r in 1 2 3; do OMP=0 run vanilla "$r"; done
