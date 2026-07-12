# Command palette — agent chat (Vercel AI SDK)

The palette's **Agent** mode is a multi-turn streaming chat assistant. The ReAct
loop runs **client-side** in the webview; native actions and the hub Search API
stay in Rust behind `invoke` commands.

## Pieces

- **`src/lib/agent/index.ts`** — the whole agent wiring:
  - `createAgentTransport(getModel)` returns a custom AI SDK `ChatTransport`
    whose `sendMessages` runs `streamText(...)` against the NeuralDeep hub
    (`@ai-sdk/openai-compatible`, `baseURL = <hub_url>/v1`, `apiKey = hub_token`
    from `getAppSettings`) with `stopWhen: stepCountIs(12)` and returns
    `toUIMessageStream({ stream, sendReasoning: true })`.
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
  - History persists full `UIMessage[]` arrays in `localStorage`
    (`agentSessions`); opening one restores the whole conversation.

## Rust side

- Tool bridges: `commands::agent_web_search` (hub creds from DB, never JS),
  `agent_create_note` / `agent_create_reminder` (ISO `due` parsed via
  `agent::parse_due_offset_secs`) / `agent_list_reminders` / `agent_read_calendar`;
  `agent_capture_active_window` (base64 PNG of `PALETTE_TARGET_PID`).
- **Capability:** all of the above plus `get_app_settings` / `hub_list_models`
  are allowed for the `command_palette` window in
  `permissions/palette-commands.toml`. If a new palette command is added, it MUST
  be listed there or the `invoke` will be rejected by the ACL.

## Notes

- The hub token is read into JS (it already was, for the model list) — acceptable
  for a local desktop app; no new exposure.
- The old Rust `agent.rs` streaming loop (`palette_agent` + `agent-progress`/
  `agent-final` events) is no longer used by the palette but is still registered.
- Web-search mode is still single-shot (`palette_search`).
- Collapsed palette = an amorphous morphing CSS "blob" (`.min-blob`), not a dot.
