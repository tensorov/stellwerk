// N2-00f: единственное расширение Pi внутри песочницы: два хостовых AF_UNIX, без сети и ключа.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { connect, type Socket } from "node:net";
import {
  getCurrentSystemPrompt,
  getCurrentTools,
  Type,
  type AssistantMessage,
  type AssistantMessageEventStream,
  type Model,
  type SimpleStreamOptions,
  type TranscriptContext,
  createAssistantMessageEventStream,
} from "@earendil-works/pi-ai";
import { defineTool, type ExtensionAPI } from "@earendil-works/pi-coding-agent";

const MAX_FRAME_BYTES = 512 * 1024;
const PROVIDER = "stellwerk";
const API = "stellwerk-uds";
const HOST_TOOLS = new Set(["read", "edit", "write"]);

type Frame = { kind: string } & Record<string, unknown>;
type WireCall = { id: string; name: string; arguments: Record<string, unknown> };
type TurnMessage =
  | { role: "system"; text: string }
  | { role: "user"; text: string }
  | { role: "assistant"; text: string | null; tool_calls?: WireCall[] }
  | { role: "tool"; tool_call_id: string; name: string; text: string; is_error: boolean };

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// Отсутствие канала — ошибка загрузки расширения, а не «работаем без гейта».
function channel(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`stellwerk: ${name} is not configured`);
  return value;
}

const MODEL_SOCK = channel("STELLWERK_MODEL_SOCK");
const GATE_SOCK = channel("STELLWERK_GATE_SOCK");

function exchange(path: string, frame: Frame, terminal: (frame: Frame) => boolean): Promise<Frame[]> {
  return new Promise((resolve, reject) => {
    const socket: Socket = connect(path);
    const frames: Frame[] = [];
    let buffer = "";
    let settled = false;
    const settle = (finish: () => void): void => {
      if (settled) return;
      settled = true;
      socket.destroy();
      finish();
    };
    socket.setEncoding("utf8");
    socket.once("connect", () => socket.write(`${JSON.stringify(frame)}\n`));
    socket.once("error", (error) => settle(() => reject(error)));
    socket.once("close", () => settle(() => resolve(frames)));
    socket.on("data", (chunk: string) => {
      buffer += chunk;
      if (buffer.length > MAX_FRAME_BYTES) return settle(() => reject(new Error("frame exceeds limit")));
      let newline = buffer.indexOf("\n");
      while (newline >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        let parsed: Frame;
        try { parsed = JSON.parse(line) as Frame; } catch { return settle(() => reject(new Error("frame is not JSON"))); }
        frames.push(parsed);
        if (terminal(parsed)) return settle(() => resolve(frames));
        newline = buffer.indexOf("\n");
      }
    });
  });
}

function pathOf(input: unknown): string | undefined {
  const path = (input as { path?: unknown } | undefined)?.path;
  return typeof path === "string" ? path : undefined;
}

async function verdict(frame: Frame, signal?: AbortSignal): Promise<{ block: true; reason: string } | undefined> {
  if (signal?.aborted) return { block: true, reason: "gate_aborted" };
  let answer: Frame | undefined;
  try {
    const frames = await exchange(GATE_SOCK, frame, () => true);
    answer = frames[0];
  } catch (error) {
    return { block: true, reason: `gate_unreachable:${describe(error)}` };
  }
  if (!answer) return { block: true, reason: "gate_unreachable:channel closed without an answer" };
  switch (answer.kind) {
    case "allow": return undefined;
    case "authorized": return undefined;
    case "deny": return { block: true, reason: `gate_denied:${String(answer.reason)}` };
    case "ask": return { block: true, reason: "gate_ask" };
    case "suspended": return { block: true, reason: "gate_suspended" };
    case "unclassified": return { block: true, reason: "gate_unclassified" };
    case "unrecognised_source": return { block: true, reason: "gate_unrecognised_source" };
    case "unknown_tool": return { block: true, reason: `gate_unknown_tool:${JSON.stringify(answer.held ?? [])}` };
    case "refused": return { block: true, reason: `gate_refused:${String(answer.code)}:${String(answer.message)}` };
    default: return { block: true, reason: `gate_unknown_answer:${String(answer.kind)}` };
  }
}

async function authorize(tool: string, path: string | undefined, signal?: AbortSignal): Promise<{ block: true; reason: string } | undefined> {
  return verdict({ kind: "tool_call", tool, parent_tool_call_id: null, path }, signal);
}

async function hostCall(tool: string, params: unknown, signal?: AbortSignal): Promise<{ block: true; reason: string } | undefined> {
  const argument = JSON.stringify(params);
  const frame: Frame = { kind: "tool_call", tool, parent_tool_call_id: null, path: pathOf(params), argument };
  if (tool === "write" || tool === "edit") {
    frame.content_digest = createHash("sha256").update(argument).digest("hex");
  }
  return verdict(frame, signal);
}

type ReadParams = { path: string };
type WriteParams = { path: string; content: string };
type EditParams = { path: string; edits: { oldText: string; newText: string }[] };

function readHost(params: ReadParams): string {
  return readFileSync(params.path, "utf8");
}

function writeHost(params: WriteParams): string {
  writeFileSync(params.path, params.content, "utf8");
  return `wrote ${Buffer.byteLength(params.content)} bytes to ${params.path}`;
}

function editHost(params: EditParams): string {
  let text = readFileSync(params.path, "utf8");
  for (const edit of params.edits) {
    if (!text.includes(edit.oldText)) throw new Error(`edit: oldText not found in ${params.path}`);
    text = text.replace(edit.oldText, edit.newText);
  }
  writeFileSync(params.path, text, "utf8");
  return `edited ${params.path}`;
}

function success(tool: string, path: string, text: string) {
  return { content: [{ type: "text" as const, text }], details: { tool, path } };
}

function failure(tool: string, path: string, reason: string) {
  return { content: [{ type: "text" as const, text: reason }], details: { tool, path }, isError: true };
}

type Block = { type?: unknown; text?: unknown };
type RawMessage = {
  role?: unknown;
  content?: unknown;
  toolCallId?: unknown;
  toolName?: unknown;
  isError?: unknown;
};

function textOf(content: unknown, represented: (block: Block) => boolean = () => false): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  const parts: string[] = [];
  let omitted = 0;
  for (const block of content as Block[]) {
    if (block.type === "text" && typeof block.text === "string") parts.push(block.text);
    else if (!represented(block)) omitted += 1;
  }
  const text = parts.join("");
  if (omitted === 0) return text;
  return `${text}${text.length > 0 ? " " : ""}[non-text content omitted: ${omitted}]`;
}

function callsOf(content: unknown): WireCall[] {
  if (!Array.isArray(content)) return [];
  const calls: WireCall[] = [];
  for (const block of content as (Block & { id?: unknown; name?: unknown; arguments?: unknown })[]) {
    if (block.type !== "toolCall") continue;
    calls.push({
      id: String(block.id),
      name: String(block.name),
      arguments: (block.arguments ?? {}) as Record<string, unknown>,
    });
  }
  return calls;
}

function toolsOf(context: TranscriptContext): { name: string; description: string; parameters: unknown }[] {
  return getCurrentTools(context.messages).map((tool) => ({
    name: tool.name,
    description: tool.description,
    parameters: tool.parameters,
  }));
}

function transcript(context: TranscriptContext): TurnMessage[] {
  const messages: TurnMessage[] = [];
  const system = getCurrentSystemPrompt(context.messages);
  if (system.length > 0) messages.push({ role: "system", text: system });
  for (const raw of context.messages as unknown as RawMessage[]) {
    if (raw.role === "system") continue;
    if (raw.role === "toolResult") {
      messages.push({
        role: "tool",
        tool_call_id: String(raw.toolCallId ?? ""),
        name: String(raw.toolName ?? ""),
        text: textOf(raw.content),
        is_error: raw.isError === true,
      });
      continue;
    }
    if (raw.role === "assistant") {
      const calls = callsOf(raw.content);
      const text = textOf(raw.content, (block) => block.type === "toolCall");
      if (text.length === 0 && calls.length === 0) continue;
      messages.push({
        role: "assistant",
        text: text.length === 0 ? null : text,
        ...(calls.length > 0 ? { tool_calls: calls } : {}),
      });
      continue;
    }
    const text = textOf(raw.content);
    if (raw.role === "user") {
      messages.push({ role: "user", text });
      continue;
    }
    const role = String(raw.role ?? "unknown");
    messages.push({ role: "user", text: text.length > 0 ? `[${role}] ${text}` : `[${role}]` });
  }
  return messages;
}

function streamStellwerk(model: Model<string>, context: TranscriptContext, options?: SimpleStreamOptions): AssistantMessageEventStream {
  const stream = createAssistantMessageEventStream();
  const output: AssistantMessage = {
    role: "assistant",
    content: [],
    api: model.api,
    provider: model.provider,
    model: model.id,
    usage: {
      input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    },
    stopReason: "pending",
    timestamp: Date.now(),
  };
  const finish = (reason: "stop" | "toolUse"): void => {
    output.stopReason = reason;
    stream.push({ type: "done", reason, message: output });
    stream.end();
  };
  void (async () => {
    try {
      stream.push({ type: "start", partial: output });
      const frames = await exchange(
        MODEL_SOCK,
        { kind: "turn", tools: toolsOf(context), messages: transcript(context) },
        (frame) => frame.kind === "done" || frame.kind === "error",
      );
      let called = false;
      for (const frame of frames) {
        if (frame.kind === "text" && typeof frame.text === "string") {
          output.content.push({ type: "text", text: frame.text });
          const contentIndex = output.content.length - 1;
          stream.push({ type: "text_start", contentIndex, partial: output });
          stream.push({ type: "text_delta", contentIndex, delta: frame.text, partial: output });
          stream.push({ type: "text_end", contentIndex, content: frame.text, partial: output });
        }
        if (frame.kind === "tool_call") {
          called = true;
          const toolCall = {
            type: "toolCall" as const,
            id: String(frame.id),
            name: String(frame.name),
            arguments: (frame.input ?? {}) as Record<string, unknown>,
          };
          output.content.push(toolCall);
          const contentIndex = output.content.length - 1;
          stream.push({ type: "toolcall_start", contentIndex, partial: output });
          stream.push({ type: "toolcall_end", contentIndex, toolCall, partial: output });
        }
        if (frame.kind === "error") throw new Error(`stellwerk: ${String(frame.code)}: ${String(frame.message)}`);
      }
      if (options?.signal?.aborted) throw new Error("stellwerk: turn aborted");
      const done = frames.at(-1);
      if (done?.kind !== "done") throw new Error("stellwerk: model channel closed before `done`");
      const reason = done.reason;
      if (reason !== "stop" && reason !== "tool_use") {
        throw new Error(`stellwerk: invalid done.reason: ${String(reason)}`);
      }
      if ((reason === "tool_use") !== called) {
        throw new Error(`stellwerk: done.reason ${reason} contradicts the frames`);
      }
      finish(reason === "tool_use" ? "toolUse" : "stop");
    } catch (error) {
      output.stopReason = options?.signal?.aborted ? "aborted" : "error";
      output.errorMessage = describe(error);
      stream.push({ type: "error", reason: output.stopReason, error: output });
      stream.end();
    }
  })();
  return stream;
}

export default function stellwerk(pi: ExtensionAPI): void {
  pi.registerProvider(PROVIDER, {
    baseUrl: "http://stellwerk.invalid",
    apiKey: "stellwerk-uds",
    api: API,
    models: [{
      id: "proxy",
      name: "Stellwerk proxy",
      api: API,
      reasoning: false,
      input: ["text"],
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
      contextWindow: 138336,
      maxTokens: 41500,
    }],
    streamSimple: streamStellwerk,
  });
  pi.registerTool(defineTool({
    name: "read",
    label: "Read",
    description: "Read a file from the host workspace after the Stellwerk gate approves the call.",
    parameters: Type.Object({ path: Type.String({ description: "Path to read" }) }),
    async execute(_toolCallId, params, signal) {
      const blocked = await hostCall("read", params, signal);
      if (blocked) return failure("read", params.path, blocked.reason);
      try {
        return success("read", params.path, readHost(params));
      } catch (error) {
        return failure("read", params.path, `read_failed:${describe(error)}`);
      }
    },
  }));
  pi.registerTool(defineTool({
    name: "edit",
    label: "Edit",
    description: "Edit a file in the host workspace after the Stellwerk gate approves the call.",
    parameters: Type.Object({
      path: Type.String({ description: "Path to edit" }),
      edits: Type.Array(Type.Object({
        oldText: Type.String({ description: "Exact text to replace" }),
        newText: Type.String({ description: "Replacement text" }),
      })),
    }),
    async execute(_toolCallId, params, signal) {
      const blocked = await hostCall("edit", params, signal);
      if (blocked) return failure("edit", params.path, blocked.reason);
      try {
        return success("edit", params.path, editHost(params));
      } catch (error) {
        return failure("edit", params.path, `edit_failed:${describe(error)}`);
      }
    },
  }));
  pi.registerTool(defineTool({
    name: "write",
    label: "Write",
    description: "Write a file in the host workspace after the Stellwerk gate approves the call.",
    parameters: Type.Object({
      path: Type.String({ description: "Path to write" }),
      content: Type.String({ description: "Exact bytes to write" }),
    }),
    async execute(_toolCallId, params, signal) {
      const blocked = await hostCall("write", params, signal);
      if (blocked) return failure("write", params.path, blocked.reason);
      try {
        return success("write", params.path, writeHost(params));
      } catch (error) {
        return failure("write", params.path, `write_failed:${describe(error)}`);
      }
    },
  }));
  // Хостовые инструменты сами отправляют полный вызов; второй пояс остаётся для
  // встроенных инструментов (bash и т.п.), которые не проходят через execute.
  pi.on("tool_call", async (event) => {
    if (HOST_TOOLS.has(event.toolName)) return undefined;
    return authorize(event.toolName, pathOf(event.input));
  });
}
