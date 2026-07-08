import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  canToggleQuickLook,
  resolveOverlayEscapeAction,
  shouldBlockOverlayActionWhileQuickLookOpen,
  shouldExitSearchToGrid,
  shouldHandleQuickLookCmdY,
  shouldHandleQuickLookSpace,
} from "./quick-look-keyboard.ts";

const idleList = {
  displayListPending: false,
  displayFetchFailed: false,
  selectedIndex: 0,
  entryCount: 3,
};

describe("canToggleQuickLook", () => {
  it("allows toggle when a card is selected and the list is ready", () => {
    assert.equal(canToggleQuickLook(idleList), true);
  });

  it("blocks toggle while the list is pending or failed", () => {
    assert.equal(canToggleQuickLook({ ...idleList, displayListPending: true }), false);
    assert.equal(canToggleQuickLook({ ...idleList, displayFetchFailed: true }), false);
  });

  it("blocks toggle without a valid selection", () => {
    assert.equal(canToggleQuickLook({ ...idleList, selectedIndex: -1 }), false);
    assert.equal(canToggleQuickLook({ ...idleList, selectedIndex: 3 }), false);
  });
});

describe("resolveOverlayEscapeAction", () => {
  it("prioritises context menu, then quick look, then search, then dismiss", () => {
    assert.equal(
      resolveOverlayEscapeAction({
        cardContextMenuOpen: true,
        quickLookOpen: true,
        hasSearchQuery: true,
      }),
      "close-context-menu",
    );
    assert.equal(
      resolveOverlayEscapeAction({
        cardContextMenuOpen: false,
        quickLookOpen: true,
        hasSearchQuery: true,
      }),
      "close-quick-look",
    );
    assert.equal(
      resolveOverlayEscapeAction({
        cardContextMenuOpen: false,
        quickLookOpen: false,
        hasSearchQuery: true,
      }),
      "clear-search",
    );
    assert.equal(
      resolveOverlayEscapeAction({
        cardContextMenuOpen: false,
        quickLookOpen: false,
        hasSearchQuery: false,
      }),
      "dismiss-overlay",
    );
  });
});

describe("shouldHandleQuickLookSpace", () => {
  it("toggles when a card is selected outside fields and buttons", () => {
    assert.equal(
      shouldHandleQuickLookSpace(" ", { altKey: false, metaKey: false, ctrlKey: false }, null, {
        ...idleList,
        searchFocused: false,
        typingInField: false,
      }),
      true,
    );
  });

  it("ignores modified Space, search focus, fields, and buttons", () => {
    const ctx = { ...idleList, searchFocused: false, typingInField: false };
    assert.equal(
      shouldHandleQuickLookSpace(" ", { altKey: true, metaKey: false, ctrlKey: false }, null, ctx),
      false,
    );
    assert.equal(
      shouldHandleQuickLookSpace(" ", { altKey: false, metaKey: false, ctrlKey: false }, null, {
        ...ctx,
        searchFocused: true,
      }),
      false,
    );
    assert.equal(
      shouldHandleQuickLookSpace(
        " ",
        { altKey: false, metaKey: false, ctrlKey: false },
        { tagName: "BUTTON" } as unknown as EventTarget,
        ctx,
      ),
      false,
    );
  });
});

describe("shouldHandleQuickLookCmdY", () => {
  it("toggles from search and ignores buttons", () => {
    assert.equal(
      shouldHandleQuickLookCmdY(
        "y",
        { altKey: false, metaKey: true, ctrlKey: false, shiftKey: false },
        null,
        idleList,
      ),
      true,
    );
    assert.equal(
      shouldHandleQuickLookCmdY(
        "y",
        { altKey: false, metaKey: true, ctrlKey: false, shiftKey: false },
        { tagName: "BUTTON" } as unknown as EventTarget,
        idleList,
      ),
      false,
    );
  });
});

describe("shouldExitSearchToGrid", () => {
  it("uses ArrowDown on vertical boards and ArrowRight/Down on horizontal boards", () => {
    assert.equal(
      shouldExitSearchToGrid(
        "ArrowDown",
        { metaKey: false, ctrlKey: false, altKey: false },
        {
          boardVertical: true,
          searchFocused: true,
        },
      ),
      true,
    );
    assert.equal(
      shouldExitSearchToGrid(
        "ArrowRight",
        { metaKey: false, ctrlKey: false, altKey: false },
        {
          boardVertical: false,
          searchFocused: true,
        },
      ),
      true,
    );
    assert.equal(
      shouldExitSearchToGrid(
        "ArrowRight",
        { metaKey: false, ctrlKey: false, altKey: false },
        {
          boardVertical: true,
          searchFocused: true,
        },
      ),
      false,
    );
  });
});

describe("shouldBlockOverlayActionWhileQuickLookOpen", () => {
  it("blocks overlay shortcuts only while preview is open", () => {
    assert.equal(shouldBlockOverlayActionWhileQuickLookOpen(true), true);
    assert.equal(shouldBlockOverlayActionWhileQuickLookOpen(false), false);
  });
});
