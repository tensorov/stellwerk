#!/usr/bin/env bash
# Тест активации: спрашиваем агента, кто он и какая у него роль/промпт.
# Если OMP загрузился — ответы будут отличаться от vanilla Pi.
set -u
ROOT=/home/izyabretatel/gits/stellwerk
PI="$ROOT/.ablation-omp/omp-install/node_modules/.bin/pi"
OMP_EXT="$ROOT/.ablation-omp/omp-install/node_modules/oh-my-pi/dist/extension.js"
SCRATCH="$ROOT/.ablation-omp/scratch-activation"; mkdir -p "$SCRATCH"
printf '{\n  "orchestrator": { "agentName": "oh-my-pi", "promptTemplate": "sisyphus" }\n}\n' > "$SCRATCH/.oh-my-pi.jsonc"
Q="Ответь строго одной строкой: как тебя зовут и какая у тебя роль (агент / координатор / субагент)?"
for arm in omp vanilla; do
  out="$ROOT/.ablation-omp/runs/activation-${arm}.jsonl"; mkdir -p "$(dirname "$out")"
  if [ "$arm" = omp ]; then EXT="-e $OMP_EXT"; else EXT=""; fi
  ( cd "$SCRATCH" && exec env PI_CODING_AGENT_SESSION_DIR="$SCRATCH/s-$arm" timeout 240 "$PI" \
      --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off \
      -p "$Q" --mode json --no-prompt-templates --no-session --no-context-files --no-skills --no-themes \
      $EXT ) > "$out" 2>"$out.stderr"
  echo "$arm exit=$?"
done
