/**
 * WebKit in Tauri often matches :focus-visible on mouse click for text fields.
 * Track last input modality so focus rings appear only after keyboard navigation.
 */
import { applySelectAllInTextField } from "./text-field-shortcuts.ts";

export function setInputModality(
  modality: "pointer" | "keyboard",
  root: HTMLElement = document.documentElement,
): void {
  root.dataset.inputModality = modality;
}

/** Drop DOM focus so rings do not persist after overlay/palette close or pointer use. */
export function blurFocusedControl(doc: Document = document): void {
  const active = doc.activeElement;
  if (
    active != null &&
    active !== doc.body &&
    typeof (active as { blur?: unknown }).blur === "function"
  ) {
    (active as unknown as { blur: () => void }).blur();
  }
}

export function resetFocusState(root: HTMLElement = document.documentElement): void {
  blurFocusedControl(root.ownerDocument ?? document);
  setInputModality("pointer", root);
}

export function shouldSetKeyboardInputModality(
  key: string,
  hasModifier: boolean,
  typingInField: boolean,
): boolean {
  if (key === "Tab") return true;
  if (typingInField) return false;
  return key.startsWith("Arrow") || key === "Enter" || key === "/" || hasModifier;
}

const POINTER_FOCUSABLE =
  "button, [role='button'], a[href], .segment-item, .filter-chip, .card.selected, .collection-tab, .tab-delete, .overlay-icon-btn, .clear-btn, .exclude-app-btn";

export function initInputModality(root: HTMLElement = document.documentElement): () => void {
  const doc = root.ownerDocument ?? document;
  setInputModality("pointer", root);

  const onPointerDown = () => setInputModality("pointer", root);

  const onPointerUp = (e: PointerEvent) => {
    if (root.dataset.inputModality !== "pointer") return;
    const target = e.target;
    const el = target instanceof Element ? target.closest(POINTER_FOCUSABLE) : null;
    if (el instanceof HTMLElement && el.matches(":focus")) {
      queueMicrotask(() => {
        if (doc.activeElement === el) el.blur();
      });
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (applySelectAllInTextField(e)) return;

    const target = e.target;
    const typingInField =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement;

    if (shouldSetKeyboardInputModality(e.key, e.metaKey || e.ctrlKey || e.altKey, typingInField)) {
      setInputModality("keyboard", root);
    }
  };

  doc.addEventListener("pointerdown", onPointerDown, true);
  doc.addEventListener("pointerup", onPointerUp, true);
  doc.addEventListener("keydown", onKeyDown, true);

  return () => {
    doc.removeEventListener("pointerdown", onPointerDown, true);
    doc.removeEventListener("pointerup", onPointerUp, true);
    doc.removeEventListener("keydown", onKeyDown, true);
    delete root.dataset.inputModality;
  };
}
