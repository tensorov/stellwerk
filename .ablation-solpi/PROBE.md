# SoL-Pi Ablation Probe — Phase 1 Findings

**Date**: 2025-07-22
**SoL-Pi commit**: `1559b5c`
**Pi version**: `0.87.0` (tested via `check-pi-compat.mjs` — exit 0)
**Base commit (stellwerk)**: `2d572f1` ("measure RED (packet-06)")

---

## 0. Compatibility Verdict (check-pi-compat.mjs)

**Command**:
```bash
cd .ablation-solpi/src/SoL-Pi && node scripts/check-pi-compat.mjs
```

**Pi version**: 0.87.0 (installed globally via `npm install --global @earendil-works/pi-coding-agent@0.87.0`)

**stdout**: (empty — script is silent on success)
**stderr**: (empty)
**exit code**: 0

**Verdict**: **COMPATIBLE**

The script verifies that the following public Pi APIs are present and callable:
- `CONFIG_DIR_NAME` (string)
- `ModelRegistry` (constructor)
- `SessionManager` (constructor)
- `createBashToolDefinition` (function)
- `createEditToolDefinition` (function)
- `createWriteToolDefinition` (function)
- `getAgentDir` (function)
- `pi-ai/compat.complete` (function)

All 8 symbols resolved to `function` or `string` in Pi 0.87.0. No errors.

---

## 1. Isolation

### Source Verification

**Remote URL**: `https://github.com/NVlabs/SoL-Pi.git`
**Owner**: `NVlabs` (canonical — not `nvidia`)
**Commit**: `1559b5c` — "Merge pull request #35 from kaluli123123/fix/action-fusion-windows-shell-paths"

The source is the canonical NVlabs/SoL-Pi repository. The earlier mention of
`github.com/nvidia/SoL-Pi` in this probe was incorrect — corrected here.

### Mechanism
SoL-Pi stores all session-derived data under the active Pi session directory:

```
<session-dir>/sol-pi/<session-id>/
```

where `<session-dir>` is obtained via `ctx.sessionManager.getSessionDir()` and
`<session-id>` via `ctx.sessionManager.getSessionId()`.

Each mechanism writes to its own subdirectory:

| Mechanism | Runtime path |
|---|---|
| Observation Pack | `<session-dir>/sol-pi/<session-id>/observation-pack/ledger.jsonl` |
| Evidence Preserving Reducer | `<session-dir>/sol-pi/<session-id>/evidence-preserving-reducer/` |
| Online Context Compact | `<session-dir>/sol-pi/<session-id>/online-context-compact/` (state) |
| Action Fusion | No persistent state |

### Isolation via `PI_CODING_AGENT_DIR`
Pi respects the `PI_CODING_AGENT_DIR` environment variable. Setting it to a
custom path redirects all agent-level config (models.json, auth.json, sessions,
etc.) to that directory. SoL-Pi's session data will then live under that
custom directory, fully isolated from production.

**Verification**:
```bash
PI_CODING_AGENT_DIR=/tmp/test-agent-dir node --input-type=module -e \
  "import { getAgentDir } from '@earendil-works/pi-coding-agent'; console.log(getAgentDir());"
# → /tmp/test-agent-dir
```

### Isolation via `pi install --local`
Project-local installation (`pi install <path> --local`) registers SoL-Pi only
in the project's `.pi/settings.json`, not globally. Combined with a separate
session directory (via `PI_CODING_AGENT_SESSION_DIR`), this provides complete
isolation.

---

## 2. Version Pinning

### Via `pi install`
SoL-Pi can be pinned by installing from a specific git ref:

```bash
# From a local checkout (project-local)
pi install ./path/to/SoL-Pi --local --approve

# From a git ref (user-wide or local)
pi install git:github.com/NVlabs/SoL-Pi#1559b5c --approve
```

The `pi list` command will show the installed source and commit.

### Via npm (alternative)
```bash
pi install npm:@earendil-works/sol-pi@<version> --approve
```

---

## 3. Activation Signals (JSON Events & TUI)

### 3.1 Action Fusion — `then_run` signal

When the agent calls `edit` or `write` with a `then_run` object, SoL-Pi fuses
the two turns (edit + command) into one. The JSON event contains:

```json
{
  "tool": "edit",
  "args": {
    "path": "src/file.rs",
    "then_run": {
      "command": "cargo test",
      "timeout_seconds": 60
    }
  }
}
```

**Detection**: Search for `then_run` in tool call JSON. If present, the edit
and command are fused — one model turn instead of two.

**TUI signal**: `⚡ SoL-Pi · Action Fusion` / `Money saved · 1 model round-trip avoided`

### 3.2 Observation Pack — `obs_recall` tool + ledger

When a large tool result (>4 KB) is sent to the model for the 3rd+ time,
SoL-Pi replaces it with a short placeholder and archives the full content.
The agent can recall it via the `obs_recall` tool.

**JSON event (ledger.jsonl)** — full send:
```json
{
  "timestamp": "2025-07-22T...",
  "event": "full",
  "id": "obs_<hash>",
  "request": 3,
  "tool": "run",
  "originalBytes": 52000,
  "originalLines": 1200,
  "originalTokens": 12000,
  "contentHash": "sha256..."
}
```

**JSON event (ledger.jsonl)** — placeholder send:
```json
{
  "timestamp": "2025-07-22T...",
  "event": "placeholder",
  "id": "obs_<hash>",
  "request": 4,
  "sendNumber": 3,
  "tool": "run",
  "originalBytes": 52000,
  "originalLines": 1200,
  "originalTokens": 12000,
  "placeholderBytes": 128,
  "placeholderTokens": 20,
  "removedTokens": 11980
}
```

**JSON event (ledger.jsonl)** — recall:
```json
{
  "timestamp": "2025-07-22T...",
  "event": "recall",
  "id": "obs_<hash>",
  "offset": 0,
  "bytes": 4096,
  "lines": 100,
  "nextOffset": 4096,
  "eof": false
}
```

**Detection**:
1. Search session logs for `obs_recall` tool calls
2. Parse `ledger.jsonl` files for `event: "placeholder"` entries
3. Count `removedTokens` across all placeholder entries

**TUI signal**: `⚡ SoL-Pi · Observation Pack` / `Money saved · <N> context tokens avoided`

### 3.3 Evidence Preserving Reducer — custom event type

When the context window is near capacity, the reducer compresses old messages
into a compact summary.

**JSON event** (in session log):
```json
{
  "type": "sol-pi-evidence-preserving-reducer-v1",
  "schema": "sol-pi-evidence-preserving-reducer/1",
  "runId": "<sha256-slice>",
  "inputMessages": [...],
  "reducedSummary": "...",
  "evidence": [
    {
      "id": "...",
      "quote": "...",
      "hash": "sha256..."
    }
  ]
}
```

**Detection**: Search session logs for `"type": "sol-pi-evidence-preserving-reducer-v1"`.

**TUI signal**: `⚡ SoL-Pi · Luna Delegating` (note: TUI label is "Luna Delegating" not "Evidence Preserving Reducer")

### 3.4 Online Context Compact — compaction decision

Monitors context window usage and decides when to trigger compaction based on
the `cacheWriteReadRatio` (default 12.5).

**JSON event** (in session log):
```json
{
  "event": "compaction",
  "reason": "cache_ratio_exceeded",
  "writeTokens": 5000,
  "readTokens": 45000,
  "ratio": 9.0,
  "threshold": 12.5,
  "compact": true,
  "keepRecentTokens": 32000
}
```

**Detection**: Search session logs for `"event": "compaction"` or
`"sol-pi-online-context-compact-abort-marker"` tool call.

**TUI signal**: `⚡ SoL-Pi · Online Context Compact`

---

## 4. Base Commit Identification

The base commit for the stellwerk `m0/measure` branch is:

```
2d572f1 - measure RED (packet-06)
```

This is the commit from which the ablation study should diverge. The
`m0/measure` branch was created from this point.

---

## 5. MICRO System Prompt (дословно)

Строка 16 файла `~/.local/bin/stellwerk-dispatch`:

```
MICRO='Ты micro-worker Stellwerk. Выполни ровно один packet ниже. Не читай другие packets и планы.
Хвост ответа строго: STATUS/FILES/TEST/EVIDENCE.'
```

MICRO передаётся через `--append-system-prompt` в вызов `pi`:

```bash
pi --provider neuraldeep --model qwen3.6-unlim-noreason --thinking off -p "$(cat "$PACKET")" \
   --mode json \
   --no-prompt-templates \
   --append-system-prompt "$MICRO" \
   > "$OUT.jsonl" 2>"$OUT.stderr"
```

**Ablation implication**: для изолированного запуска необходимо передать тот же
MICRO через `--append-system-prompt` или вставить в `PACKET`.

---

## 6. Security Policy — Three Lines from SECURITY.md

1. **SoL-Pi runs with the filesystem, process, network, and credential
   permissions of the Pi process that loads it. SoL-Pi is not a sandbox or
   permission boundary.**

2. **Evidence-Preserving Reducer archives diagnostic logs locally and, when
   explicitly enabled, sends eligible logs through the configured reducer model
   using Pi-managed authentication.** (The reducer skips text matching its
   likely-secret detector, but that detector is a precaution rather than a
   complete secret scanner. Do not enable remote reduction for workloads whose
   logs must remain local.)

3. **Online Context Compact stores plan and compaction state in Pi's session
   log; see below.** The latest valid entry holds the model-authored plan,
   concise progress fields, request counts, token-growth estimates, and
   compaction debt. These values can include paths, command names, and design
   notes and should be treated as sensitive as the rest of the conversation.

**Ablation implication**: `evidencePreservingReducer` must remain `false`.
All other features are local-only (file modification, session log writes).

### Trust model
- SoL-Pi requires project trust (`isProjectTrusted()`) for config loading
- `pi install --local --approve` persists trust for the project
- Project-local `.pi/sol-pi.json` files should be used only in trusted
  repositories

---

## 7. Configuration Loading Order

SoL-Pi does **not** merge project and global config files. The project file
**replaces** the global file.

Resolution order (after trust check):
1. `<working-directory>/<pi-config-dir>/sol-pi.json` (project-local)
2. `<pi-agent-dir>/sol-pi.json` (user-wide)
3. Built-in defaults (`DEFAULT_CONFIG`)

**Key**: `CONFIG_DIR_NAME` = `.pi` (from Pi's package.json `piConfig.configDir`)

---

## 8. Default Configuration

```json
{
  "version": 1,
  "actionFusion": false,
  "observationPack": false,
  "evidencePreservingReducer": false,
  "evidencePreservingReducerProvider": "openai-codex",
  "evidencePreservingReducerModel": "gpt-5.6-luna",
  "onlineContextCompact": false,
  "cacheWriteReadRatio": 12.5
}
```

All features are **disabled by default**. The `cacheWriteReadRatio` defaults to
`12.5` (OpenAI GPT-5.6 Sol OpenAI Standard cache-write/read ratio).

---

## 9. Key Source Files

| File | Purpose |
|---|---|
| `src/sol-pi/index.ts` | Extension factory, config loading, feature registration |
| `src/sol-pi/config.ts` | `SolPiConfig` interface, `DEFAULT_CONFIG`, `findConfigPath` |
| `src/sol-pi/runtime-paths.ts` | `runtimeRoot()` — session-derived path construction |
| `src/sol-pi/tui.ts` | TUI rendering, `showSolPiSavings()`, status key `sol-pi-savings` |
| `src/sol-pi/extensions/action-fusion/index.ts` | `edit`/`write` tool override with `then_run` |
| `src/sol-pi/extensions/observation-pack/index.ts` | `obs_recall` tool, `context` hook, ledger |
| `src/sol-pi/extensions/evidence-preserving-reducer/index.ts` | Context compression |
| `src/sol-pi/extensions/online-context-compact/index.ts` | Compaction decision engine |

---

## 10. Next Steps (Phase 2)

1. Create isolated `PI_CODING_AGENT_DIR` environment
2. Install SoL-Pi via `pi install` with version pin
3. Extract and verify MICRO prompt in isolation
4. Run baseline (no SoL-Pi) and ablation (with SoL-Pi) on packet-07a
5. Compare context usage, steps, interruptions, and results
