# PROBE — Pi Worker Environment Baseline

> **Date**: 2026-09-22
> **Project**: stellwerk (Rust workspace: `stellwerk-core`, `stellwerk-registry`)
> **Reference**: OpenCode V9 baseline = 8426 tokens

## 1. Pi Installation

| Property | Value |
|----------|-------|
| **Binary** | `/home/izyabretatel/.pi/agent/bin/pi` |
| **Version** | 0.87.0 |
| **Package** | `@earendil-works/pi-coding-agent` (v0.87.0) |
| **Install method** | npm (managed, `~/.pi/agent/install/`) |
| **Node.js** | v22.23.2 |
| **npm** | 12.0.2 |
| **Agent dir** | `~/.pi/agent/` (152 MB) |
| **Config dir** | `~/.pi/agent/` (settings.json, auth.json, models.json) |

## 2. Model & Provider

| Property | Value |
|----------|-------|
| **Provider** | neuraldeep |
| **Model (current)** | `qwen3.6-unlim-noreason` |
| **Model (original)** | `qwen3.6-unlim` |
| **Reasoning** | off |
| **Context window** | 46112 tokens |
| **Max output** | 8000 tokens |
| **Available models** | 2 (see models.json) |

## 3. Configuration

### 3.1 Config Files

| File | Path | Purpose |
|------|------|---------|
| settings.json | `~/.pi/agent/settings.json` | Theme (dark), changelog version |
| auth.json | `~/.pi/agent/auth.json` | Provider credentials (neuraldeep, opencode) |
| models.json | `~/.pi/agent/models.json` | Model registry (neuraldeep provider) |

### 3.2 Environment Variables

| Variable | Value | Purpose |
|----------|-------|---------|
| `PI_CODING_AGENT` | `true` | Process marker |
| `PI_SESSION_FILE` | `~/.pi/agent/sessions/--home-izyabretatel-gits-stellwerk--/2026-09-22T11-27-19-638Z_01a0c8de-cc55-7016-b543-9b4139741af9.jsonl` | Current session |
| `PI_SESSION_ID` | `01a0c8de-cc55-7016-b543-9b4139741af9` | Current session UUID |
| `PI_PROVIDER` | `neuraldeep` | Current provider |
| `PI_MODEL` | `qwen3.6-unlim-noreason` | Current model |
| `PI_REASONING_LEVEL` | `off` | Current reasoning level |
| `PI_MANAGED_INSTALL_ROOT` | `~/.local/bin/../../.pi/agent/install` | Install root |
| `PI_CODING_AGENT_DIR` | *(not set, defaults to ~/.pi/agent)* | Override config dir |
| `PI_CODING_AGENT_SESSION_DIR` | *(not set)* | Override session dir |
| `PI_PACKAGE_DIR` | *(not set)* | Override package dir |

### 3.3 Isolation Support

Pi supports full isolation via environment variables:

- **`PI_CODING_AGENT_DIR`** — Override the entire config directory (default `~/.pi/agent`)
- **`PI_CODING_AGENT_SESSION_DIR`** — Override session storage
- **`--session-dir <path>`** — CLI flag for session directory
- **`--no-context-files, -nc`** — Disable AGENTS.md / CLAUDE.md discovery
- **`--no-tools, -nt`** — Disable all tools by default
- **`--tools <list>`** — Explicit allowlist of tools
- **`--exclude-tools, -xt <list>`** — Explicit denylist

## 4. Built-in Tools

| Tool | Description | Default |
|------|-------------|---------|
| `read` | Read file contents (text + images) | enabled |
| `bash` | Execute bash commands | enabled |
| `powershell` | Execute PowerShell (Windows) | enabled |
| `edit` | Edit files with find/replace | enabled |
| `write` | Write/overwrite files | enabled |
| `grep` | Search file contents (read-only) | **off** |
| `find` | Find files by glob (read-only) | **off** |
| `ls` | List directory contents (read-only) | **off** |

## 5. Modes & Interfaces

| Mode | Description | Token/Usage Info |
|------|-------------|------------------|
| **interactive** | Default TUI mode | N/A |
| **print (`-p`)** | Non-interactive: process prompt and exit | N/A |
| **json** | JSON event stream on stdout | ✅ `usage` field in `message_update` events |
| **rpc** | JSON protocol over stdin/stdout | ✅ Full event stream with `usage` |
| **export** | Export session to HTML/JSON | N/A |

### 5.1 JSON Mode — Token Extraction

JSON mode (`--mode json`) outputs all session events as JSONL to stdout. Each `message_update` event includes a `usage` field:

```json
{"type":"message_update","usage":{...},"assistantMessageEvent":{"type":"text_delta",...}}
```

### 5.2 RPC Mode — Token Extraction

RPC mode (`--mode rpc`) provides full programmatic control via stdin/stdout JSON protocol. Events include `usage` tracking.

### 5.3 Session Format — Token Extraction

Session files (JSONL, version 3) store full `Usage` objects in `AssistantMessage` entries:

```typescript
interface Usage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  cacheWrite1h?: number;
  reasoning?: number;
  totalTokens: number;
  cost: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
    total: number;
  };
}
```

**This is the primary mechanism for automated token extraction.**

## 6. MCP Status

| Component | Version | Status |
|-----------|---------|--------|
| `mcp-chrome-bridge` | 1.0.31 | ✅ Globally installed |
| `pi-mcp-adapter` | 2.36.0 (npm) | ❌ Not globally installed |
| `@earendil-works/pi-mcp-adapter` | — | ❌ Does not exist on npm |

**Note**: Pi documentation states: *"It intentionally does not include built-in MCP"*. MCP must be added via extensions or external tools.

## 7. Key Capabilities for Measurement

### 7.1 Token Tracking (Confirmed)
- ✅ RPC mode provides real-time `usage` events
- ✅ JSON mode provides real-time `usage` events
- ✅ Session files store full `Usage` objects (input, output, cache, cost)
- ✅ SDK `AgentSession` exposes usage via events

### 7.2 Headless Operation (Confirmed)
- ✅ `--mode json` — non-interactive JSON event stream
- ✅ `--mode rpc` — programmatic JSON protocol
- ✅ `--print` / `-p` — simple non-interactive mode
- ✅ `--no-context-files` — disable AGENTS.md / CLAUDE.md
- ✅ `--no-tools` / `--tools` — tool control
- ✅ `--session` / `--no-session` — session control
- ✅ `PI_CODING_AGENT_DIR` — full config isolation

### 7.3 Prompt Engineering (Confirmed)
- ✅ `--system-prompt <text>` — override system prompt
- ✅ `--append-system-prompt <text>` — append to system prompt
- ✅ `--prompt-template <path>` — load prompt templates
- ✅ `--no-prompt-templates` — disable template discovery
- ✅ `--model <pattern>` — model selection
- ✅ `--provider <name>` — provider selection

## 8. Constraints & Limitations

| Constraint | Impact |
|------------|--------|
| No built-in MCP | MCP integration requires external adapter |
| Context files (AGENTS.md) auto-loaded | Must use `-nc` to disable for clean baseline |
| Skills auto-loaded | May add overhead; check if skippable |
| Extension discovery | May add overhead; check if skippable |
| Session persistence by default | Use `--no-session` for ephemeral runs |
| Provider-specific usage reporting | Some providers may only report at completion |

## 9. Probe Summary

Pi v0.87.0 is a mature, headless-capable coding agent with:
- Full RPC/JSON modes for programmatic interaction
- Built-in token usage tracking (input, output, cache, cost)
- Session file persistence with full `Usage` objects
- Config isolation via `PI_CODING_AGENT_DIR`
- Prompt engineering via CLI flags
- No built-in MCP (requires external adapter)

**Verdict**: Pi is suitable for automated token measurement harness. The primary measurement mechanism will be RPC/JSON mode with `usage` event extraction, validated against session file `Usage` objects.

## 10. Measurement Results (Phase 2)

Harness: `.ablation/pi/scripts/measure-pi.sh`
Extractor: `.ablation/pi/scripts/extract-pi-usage.py`
Ledger: `.ablation/pi/ledger.tsv`
Baseline (OpenCode V9): 8426 tokens

| Variant | Description | Median context_input | Spread | Stability |
|---------|-------------|---------------------|--------|-----------|
| P0 | No tools, no prompt templates | 463 | 0.0% | STABLE |
| P1 | Default tools + skills (no system prompt) | 2966 | 0.0% | STABLE |
| P2 | Default tools + skills + custom PI.md system prompt | 2426 | 0.0% | STABLE |
| P3 | Custom PI.md system prompt, no tools | 131 | 0.0% | STABLE |

### Key Findings

- **P0 (463 tokens)**: Minimal overhead — just the system preamble + tools definitions. No skills loaded.
- **P1 (2966 tokens)**: Adding skills auto-loading adds ~2500 tokens. The skills section in the system prompt is large.
- **P2 (2426 tokens)**: Custom system prompt replaces the default one, saving ~540 tokens vs P1 (skills still loaded).
- **P3 (131 tokens)**: No tools + custom prompt = minimal context. The prompt is just the PI.md content.
- **Cache behavior**: P0/P3 show no cache hits (cold start). P1/P2 show cacheRead=2096 in some runs.
- **All variants are STABLE** (0% spread across 3 runs).

### Cost Comparison vs OpenCode V9 (8426 tokens)

| Variant | Tokens | Savings vs V9 | Cost ratio |
|---------|--------|---------------|------------|
| P0 | 463 | 94.5% | 0.055x |
| P1 | 2966 | 64.8% | 0.352x |
| P2 | 2426 | 71.2% | 0.288x |
| P3 | 131 | 98.4% | 0.016x |

**P3 is the most cost-efficient** at 131 tokens (98.4% savings), but lacks tool access.
P0 at 463 tokens (94.5% savings) provides full tool access with minimal overhead.

---

*End of PROBE.md*
