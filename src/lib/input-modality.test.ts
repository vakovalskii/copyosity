import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  blurFocusedControl,
  resetFocusState,
  setInputModality,
  shouldSetKeyboardInputModality,
} from "./input-modality.ts";

function mockRoot(): HTMLElement {
  return { dataset: {} } as HTMLElement;
}

function mockDocument(activeElement: Element | null): Document {
  const body = {} as HTMLBodyElement;
  return { activeElement, body } as unknown as Document;
}

describe("setInputModality", () => {
  it("writes keyboard and pointer to data-input-modality", () => {
    const root = mockRoot();
    setInputModality("keyboard", root);
    assert.equal(root.dataset.inputModality, "keyboard");
    setInputModality("pointer", root);
    assert.equal(root.dataset.inputModality, "pointer");
  });
});

describe("shouldSetKeyboardInputModality", () => {
  it("recognizes keyboard navigation outside text fields", () => {
    assert.equal(shouldSetKeyboardInputModality("Tab", false, false), true);
    assert.equal(shouldSetKeyboardInputModality("ArrowRight", false, false), true);
    assert.equal(shouldSetKeyboardInputModality("/", false, false), true);
    assert.equal(shouldSetKeyboardInputModality("f", true, false), true);
  });

  it("ignores typing and plain character input", () => {
    assert.equal(shouldSetKeyboardInputModality("ArrowRight", false, true), false);
    assert.equal(shouldSetKeyboardInputModality("f", true, true), false);
    assert.equal(shouldSetKeyboardInputModality("x", false, false), false);
  });
});

describe("blurFocusedControl", () => {
  it("blurs the active element when it is not body", () => {
    let blurred = false;
    const button = {
      blur: () => {
        blurred = true;
      },
    } as unknown as HTMLElement;
    blurFocusedControl(mockDocument(button));
    assert.equal(blurred, true);
  });

  it("does not call blur when body is focused", () => {
    let blurred = false;
    const body = {
      blur: () => {
        blurred = true;
      },
    } as unknown as HTMLBodyElement;
    const doc = { activeElement: body, body } as unknown as Document;
    blurFocusedControl(doc);
    assert.equal(blurred, false);
  });

  it("does not throw when there is no active element", () => {
    assert.doesNotThrow(() => blurFocusedControl(mockDocument(null)));
  });
});

describe("resetFocusState", () => {
  it("blurs the focused control and resets modality to pointer", () => {
    let blurred = false;
    const button = {
      blur: () => {
        blurred = true;
      },
    } as unknown as HTMLElement;
    const root = {
      dataset: { inputModality: "keyboard" },
      ownerDocument: mockDocument(button),
    } as unknown as HTMLElement;

    resetFocusState(root);

    assert.equal(blurred, true);
    assert.equal(root.dataset.inputModality, "pointer");
  });
});
