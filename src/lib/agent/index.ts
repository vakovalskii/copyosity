// Client-side ReAct agent for the command palette, built on the Vercel AI SDK.
//
// The loop runs in the webview: `streamText` drives a multi-step tool loop
// against the NeuralDeep hub (OpenAI-compatible), and native tools call back
// into Rust via Tauri `invoke`. The hub base URL + token are read from app
// settings (already exposed to the frontend for the model list).
//
// Reasoning: the hub returns `reasoning_content`, which `@ai-sdk/openai-compatible`
// maps to a `reasoning` UI part automatically, so the UI can render a collapsible
// "thinking" block. Tool calls surface as `tool-<name>` parts.

import { getAppSettings } from "$lib/api";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { invoke } from "@tauri-apps/api/core";
import {
  streamText,
  convertToModelMessages,
  stepCountIs,
  toUIMessageStream,
  tool,
  type UIMessage,
  type ChatTransport,
} from "ai";
import { z } from "zod";

const MAX_STEPS = 12;

const SYSTEM_PROMPT =
  "You are a personal assistant agent on the user's Mac. You can search the web AND act " +
  "on the user's apps: create notes (create_note), create/list reminders (create_reminder, " +
  "list_reminders), and read their calendar (read_calendar). Use the right tool for the " +
  "request — e.g. 'remind me tomorrow at 10 to call Bob' -> create_reminder; 'what's on my " +
  "calendar this week' -> read_calendar; 'save this to notes' -> create_note. When a screenshot " +
  "is attached, use it as context for the question. Call tools as needed, then give a concise " +
  "answer/confirmation in the user's language. Do not invent facts — use web_search.";

/** Native + hub-backed tools, each bridging to a Rust command. */
const tools = {
  web_search: tool({
    description:
      "Search the web for current/factual information. Use it whenever the question needs " +
      "fresh facts, news, prices, docs or anything you are unsure about.",
    inputSchema: z.object({ query: z.string().describe("search query") }),
    execute: ({ query }) => invoke<string>("agent_web_search", { query, limit: 5 }),
  }),
  create_note: tool({
    description: "Create a note in the user's macOS Notes app.",
    inputSchema: z.object({ title: z.string(), body: z.string() }),
    execute: ({ title, body }) => invoke<string>("agent_create_note", { title, body }),
  }),
  create_reminder: tool({
    description: "Create a reminder in the user's macOS Reminders app.",
    inputSchema: z.object({
      title: z.string(),
      due: z
        .string()
        .optional()
        .describe("optional due date/time as ISO 8601 (e.g. 2026-06-20T10:00:00)"),
    }),
    execute: ({ title, due }) =>
      invoke<string>("agent_create_reminder", { title, due: due ?? null }),
  }),
  list_reminders: tool({
    description: "List the user's open (incomplete) reminders.",
    inputSchema: z.object({}),
    execute: () => invoke<string>("agent_list_reminders"),
  }),
  read_calendar: tool({
    description: "Read the user's upcoming macOS Calendar events for the next N days.",
    inputSchema: z.object({ days: z.number().int().describe("how many days ahead (1-60)") }),
    execute: ({ days }) => invoke<string>("agent_read_calendar", { days }),
  }),
};

/** Build the OpenAI-compatible model for the current hub settings + model id. */
async function resolveModel(model: string) {
  const s = await getAppSettings();
  if (!s.hub_enabled) throw new Error("NeuralDeep hub is disabled in Settings");
  const base = s.hub_url.trim().replace(/\/+$/, "");
  if (!base || !s.hub_token.trim()) {
    throw new Error("Set the NeuralDeep hub URL and token in Settings");
  }
  const provider = createOpenAICompatible({
    name: "neuraldeep",
    baseURL: `${base}/v1`,
    apiKey: s.hub_token.trim(),
  });
  const id = model.trim() || s.hub_chat_model || "qwen3.6-35b-a3b";
  return provider.chatModel(id);
}

/**
 * A ChatTransport that runs the whole ReAct loop client-side via `streamText`
 * and streams UI message chunks (text, reasoning, tool parts) back to `Chat`.
 * `getModel` returns the currently-selected model id at send time.
 */
export function createAgentTransport(getModel: () => string): ChatTransport<UIMessage> {
  return {
    async sendMessages({ messages, abortSignal }) {
      const model = await resolveModel(getModel());
      const result = streamText({
        model,
        system: SYSTEM_PROMPT,
        messages: await convertToModelMessages(messages),
        tools,
        stopWhen: stepCountIs(MAX_STEPS),
        temperature: 0.2,
        abortSignal,
      });
      return toUIMessageStream({ stream: result.stream, sendReasoning: true });
    },
    async reconnectToStream() {
      return null;
    },
  };
}

/** Capture the active window as a data URL for image context (null if unavailable). */
export async function captureActiveWindowDataUrl(): Promise<string | null> {
  try {
    const b64 = await invoke<string | null>("agent_capture_active_window");
    return b64 ? `data:image/png;base64,${b64}` : null;
  } catch {
    return null;
  }
}
