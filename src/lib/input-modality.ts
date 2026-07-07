/**
 * WebKit in Tauri often matches :focus-visible on mouse click for text fields.
 * Track last input modality so focus rings appear only after keyboard navigation.
 */
import { applySelectAllInTextField } from "$lib/text-field-shortcuts";

export function setInputModality(
  modality: "pointer" | "keyboard",
  root: HTMLElement = document.documentElement,
): void {
  root.dataset.inputModality = modality;
}

export function initInputModality(root: HTMLElement = document.documentElement): () => void {
  const onPointerDown = () => setInputModality("pointer", root);

  const onKeyDown = (e: KeyboardEvent) => {
    if (applySelectAllInTextField(e)) return;

    const target = e.target;
    const typingInField =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement;

    if (e.key === "Tab") {
      setInputModality("keyboard", root);
      return;
    }

    if (e.key.startsWith("Arrow") || e.key === "Enter") {
      if (!typingInField) setInputModality("keyboard", root);
      return;
    }

    // Panel shortcuts (⌘F, paste, etc.) outside a text field still deserve the keyboard ring.
    if ((e.metaKey || e.ctrlKey || e.altKey) && !typingInField) {
      setInputModality("keyboard", root);
      return;
    }

    if (e.key === "/" && !typingInField) {
      setInputModality("keyboard", root);
    }
  };

  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("keydown", onKeyDown, true);

  return () => {
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("keydown", onKeyDown, true);
    delete root.dataset.inputModality;
  };
}
