import type { UIMessage } from "ai";

export type HubAgentSettings = {
  hub_enabled: boolean;
  hub_url: string;
  hub_token: string;
};

/** When non-null, the agent must not send — same checks as web search on the backend. */
export function hubAgentBlockReason(s: HubAgentSettings): string | null {
  if (!s.hub_enabled) return "NeuralDeep hub is disabled in Settings";
  const base = s.hub_url.trim().replace(/\/+$/, "");
  if (!base || !s.hub_token.trim()) {
    return "Set the NeuralDeep hub URL and token in Settings";
  }
  return null;
}

/** Drop trailing user-only messages (failed sends with no assistant reply). */
export function trimOrphanedUserMessages(messages: UIMessage[]): UIMessage[] {
  let end = messages.length;
  while (end > 0 && messages[end - 1]?.role === "user") end--;
  return end === messages.length ? messages : messages.slice(0, end);
}

/** Persist only conversations that contain a completed assistant turn. */
export function sessionCanPersist(messages: UIMessage[]): boolean {
  return messages.some((message) => message.role === "assistant");
}
