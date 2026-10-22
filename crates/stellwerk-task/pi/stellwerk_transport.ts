// N2-00f: единственное расширение Pi внутри песочницы: два хостовых AF_UNIX, без сети и ключа.
import { connect, type Socket } from "node:net";
import {
  type AssistantMessage,
  type AssistantMessageEventStream,
  type Model,
  type SimpleStreamOptions,
  type TranscriptContext,
  createAssistantMessageEventStream,
} from "@earendil-works/pi-ai";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const MAX_FRAME_BYTES = 64 * 1024;
const PROVIDER = "stellwerk";
const API = "stellwerk-uds";

type Frame = { kind: string } & Record<string, unknown>;
type TurnMessage = { role: "user" | "assistant"; text: string };

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

async function authorize(tool: string, path: string | undefined, signal?: AbortSignal): Promise<{ block: true; reason: string } | undefined> {
  if (signal?.aborted) return { block: true, reason: "gate_aborted" };
  let answer: Frame | undefined;
  try {
    const frames = await exchange(GATE_SOCK, { kind: "tool_call", tool, parent_tool_call_id: null, path }, () => true);
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

type Block = { type?: unknown; text?: unknown };
type ContentBlock = { role?: unknown; content?: unknown };

function textOf(content: unknown): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .map((block) => block as Block)
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text as string)
    .join("");
}

function transcript(context: TranscriptContext): TurnMessage[] {
  const messages: TurnMessage[] = [];
  for (const raw of context.messages as unknown as ContentBlock[]) {
    const text = textOf(raw.content);
    if (text.length === 0) continue;
    if (raw.role === "user" || raw.role === "assistant") messages.push({ role: raw.role, text });
    else messages.push({ role: "user", text: `[${String(raw.role ?? "unknown")}] ${text}` });
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
      const frames = await exchange(MODEL_SOCK, { kind: "turn", messages: transcript(context) }, (frame) => frame.kind === "done" || frame.kind === "error");
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
      if (frames.at(-1)?.kind !== "done") throw new Error("stellwerk: model channel closed before `done`");
      finish(called ? "toolUse" : "stop");
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
  pi.on("tool_call", async (event) => authorize(event.toolName, pathOf(event.input)));
}
