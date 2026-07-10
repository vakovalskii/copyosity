import assert from "node:assert/strict";
import { afterEach, beforeEach, describe, it } from "node:test";

import {
  closeAllCardContextMenus,
  isCardContextMenuOpen,
  notifyCardContextMenuClosed,
  notifyCardContextMenuOpened,
  OVERLAY_CLOSE_CARD_CONTEXT_MENUS,
} from "./overlay-card-context-menu.ts";

// Module-level open count persists across tests; reset by draining it symmetrically.
function resetOpenCount() {
  while (isCardContextMenuOpen()) notifyCardContextMenuClosed();
}

// node:test has no DOM; the module only needs dispatchEvent/CustomEvent from `window`.
const originalWindow = (globalThis as { window?: unknown }).window;

beforeEach(() => {
  resetOpenCount();
  (globalThis as { window: EventTarget }).window = new EventTarget();
});

afterEach(() => {
  resetOpenCount();
  (globalThis as { window?: unknown }).window = originalWindow;
});

describe("isCardContextMenuOpen", () => {
  it("is false when no menu has been opened", () => {
    assert.equal(isCardContextMenuOpen(), false);
  });

  it("is true once a menu opens and false after it closes", () => {
    notifyCardContextMenuOpened();
    assert.equal(isCardContextMenuOpen(), true);
    notifyCardContextMenuClosed();
    assert.equal(isCardContextMenuOpen(), false);
  });

  it("tracks multiple concurrently open menus", () => {
    notifyCardContextMenuOpened();
    notifyCardContextMenuOpened();
    assert.equal(isCardContextMenuOpen(), true);
    notifyCardContextMenuClosed();
    assert.equal(isCardContextMenuOpen(), true, "one menu is still open");
    notifyCardContextMenuClosed();
    assert.equal(isCardContextMenuOpen(), false);
  });

  it("never goes negative on an unmatched close", () => {
    notifyCardContextMenuClosed();
    notifyCardContextMenuClosed();
    assert.equal(isCardContextMenuOpen(), false);
    notifyCardContextMenuOpened();
    assert.equal(isCardContextMenuOpen(), true);
  });
});

describe("closeAllCardContextMenus", () => {
  it("dispatches the close event when a menu is open", () => {
    notifyCardContextMenuOpened();
    let received = 0;
    const handler = () => {
      received += 1;
    };
    window.addEventListener(OVERLAY_CLOSE_CARD_CONTEXT_MENUS, handler);
    try {
      closeAllCardContextMenus();
    } finally {
      window.removeEventListener(OVERLAY_CLOSE_CARD_CONTEXT_MENUS, handler);
    }
    assert.equal(received, 1);
  });

  it("does not dispatch when no menu is open", () => {
    let received = 0;
    const handler = () => {
      received += 1;
    };
    window.addEventListener(OVERLAY_CLOSE_CARD_CONTEXT_MENUS, handler);
    try {
      closeAllCardContextMenus();
    } finally {
      window.removeEventListener(OVERLAY_CLOSE_CARD_CONTEXT_MENUS, handler);
    }
    assert.equal(received, 0);
  });
});
