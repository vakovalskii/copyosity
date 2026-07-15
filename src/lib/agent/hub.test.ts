import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { UIMessage } from "ai";

import { hubAgentBlockReason, sessionCanPersist, trimOrphanedUserMessages } from "./hub.ts";

const user = (id: string): UIMessage => ({
  id,
  role: "user",
  parts: [{ type: "text", text: id }],
});
const assistant = (id: string): UIMessage => ({
  id,
  role: "assistant",
  parts: [{ type: "text", text: id }],
});

describe("hubAgentBlockReason", () => {
  it("blocks when hub is disabled", () => {
    assert.equal(
      hubAgentBlockReason({ hub_enabled: false, hub_url: "https://hub", hub_token: "tok" }),
      "NeuralDeep hub is disabled in Settings",
    );
  });

  it("blocks when url or token is missing", () => {
    assert.equal(
      hubAgentBlockReason({ hub_enabled: true, hub_url: "", hub_token: "tok" }),
      "Set the NeuralDeep hub URL and token in Settings",
    );
    assert.equal(
      hubAgentBlockReason({ hub_enabled: true, hub_url: "https://hub", hub_token: "  " }),
      "Set the NeuralDeep hub URL and token in Settings",
    );
    assert.equal(
      hubAgentBlockReason({ hub_enabled: true, hub_url: "   ", hub_token: "tok" }),
      "Set the NeuralDeep hub URL and token in Settings",
    );
  });

  it("allows configured hub", () => {
    assert.equal(
      hubAgentBlockReason({ hub_enabled: true, hub_url: "https://hub/", hub_token: "tok" }),
      null,
    );
  });
});

describe("trimOrphanedUserMessages", () => {
  it("removes trailing user-only messages", () => {
    assert.deepEqual(trimOrphanedUserMessages([user("1"), assistant("2"), user("3"), user("4")]), [
      user("1"),
      assistant("2"),
    ]);
  });

  it("removes an all-user thread without mutating it", () => {
    const messages = [user("1"), user("2")];
    assert.deepEqual(trimOrphanedUserMessages(messages), []);
    assert.deepEqual(messages, [user("1"), user("2")]);
  });

  it("keeps completed conversations intact", () => {
    const msgs = [user("1"), assistant("2")];
    assert.equal(trimOrphanedUserMessages(msgs), msgs);
  });
});

describe("sessionCanPersist", () => {
  it("rejects empty and user-only sessions", () => {
    assert.equal(sessionCanPersist([]), false);
    assert.equal(sessionCanPersist([user("1")]), false);
  });

  it("accepts a session after an assistant reply", () => {
    assert.equal(sessionCanPersist([user("1"), assistant("2")]), true);
  });
});
