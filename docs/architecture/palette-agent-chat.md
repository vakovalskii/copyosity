# Command palette — agent chat (Vercel AI SDK)

The palette's **Agent** mode is a multi-turn streaming chat assistant. The ReAct
loop runs **client-side** in the webview; native actions and the hub Search API
stay in Rust behind `invoke` commands.

## Pieces

- **`src/lib/agent/hub.ts`** — shared hub gate + session helpers (covered by
  `hub.test.ts`):
  - `hubAgentBlockReason(settings)` — returns a user-facing block string when
    hub is off / URL / token missing (same checks as backend web search). Used
    by `resolveModel` and the palette send path so failures surface **up front**,
    not mid-request.
  - `trimOrphanedUserMessages(messages)` — drops trailing user-only tails (failed
    sends with no assistant reply).
  - `sessionCanPersist(messages)` — true only when an assistant message exists;
    user-only threads are never written to history.
- **`src/lib/agent/index.ts`** — the whole agent wiring:
  - `createAgentTransport(getModel)` returns a custom AI SDK `ChatTransport`
    whose `sendMessages` runs `streamText(...)` against the NeuralDeep hub
    (`@ai-sdk/openai-compatible`, `baseURL = <hub_url>/v1`, `apiKey = hub_token`
    from `getAppSettings`) with `stopWhen: stepCountIs(12)` and returns
    `toUIMessageStream({ stream, sendReasoning: true })`.
  - Uses `hubAgentBlockReason` inside `resolveModel` before building the provider.
  - `tools` — five AI SDK tools whose `execute` calls Rust commands:
    `web_search`, `create_note`, `create_reminder`, `list_reminders`,
    `read_calendar`.
  - `captureActiveWindowDataUrl()` — screenshot of the pre-palette frontmost
    window (Rust `agent_capture_active_window`), attached as a file part.
- **`src/routes/palette/+page.svelte`** — the chat UI: `new Chat({ transport })`,
  input at the **bottom** (auto-grow textarea, Enter=send / Shift+Enter=newline),
  transcript above it. Renders `message.parts`:
  - `text` → markdown bubble
  - `reasoning` → collapsible "Reasoning" block (hub returns `reasoning_content`,
    mapped to a reasoning part automatically by the openai-compatible provider)
  - `tool-<name>` → collapsible tool chip (input/output, running/done/error)
  - Stop button while streaming; typing indicator before the first token.
  - **Status rail** above the composer (`aria-live="polite"`) for progress,
    errors, and hub-block notices; center placeholders hide while a notice is
    shown.
  - **Failed-turn recovery** — `revertFailedUserTurn()` trims orphan user
    messages and restores composer text on send/`onError` failure.
  - **Session history** — persists full `UIMessage[]` in `localStorage`
    (`agentSessions`) only when `sessionCanPersist` is true; opening one restores
    the whole conversation.
  - **Hub settings sync** — refreshes on mount, `palette-show`, and
    `hub-settings-changed` (Settings emits after Save / hub master toggle);
    re-checks block reason before each agent send.
  - **Focus** — `resetFocusState()` on hide / minimize so modality-gated focus
    rings do not linger; Rust `hide_command_palette` emits `palette-hide`.

## Rust side

- Tool bridges: `commands::agent_web_search` (hub creds from DB, never JS),
  `agent_create_note` / `agent_create_reminder` (ISO `due` parsed via
  `agent::parse_due_offset_secs`) / `agent_list_reminders` / `agent_read_calendar`;
  `agent_capture_active_window` (base64 PNG of `PALETTE_TARGET_PID`).
- **Capability:** all of the above plus `get_app_settings` / `hub_list_models`
  are allowed for the `command_palette` window in
  `permissions/palette-commands.toml`. If a new palette command is added, it MUST
  be listed there or the `invoke` will be rejected by the ACL.
- Palette shortcut get/set for Settings lives in
  `permissions/settings-commands.toml` (`get_palette_shortcut` /
  `set_palette_shortcut`).

## Notes

- The hub token is read into JS (it already was, for the model list) — acceptable
  for a local desktop app; no new exposure.
- The old Rust `agent.rs` streaming loop (`palette_agent` + `agent-progress`/
  `agent-final` events) is no longer used by the palette but is still registered.
- Web-search mode is still single-shot (`palette_search`).
- Collapsed palette = an amorphous morphing CSS "blob" (`.min-blob`), not a dot.
