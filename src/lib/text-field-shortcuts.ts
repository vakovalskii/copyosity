/** macOS default Edit menu is disabled (tray); restore select-all in text fields. */

export function isEditableTextField(
  target: EventTarget | null,
): target is HTMLInputElement | HTMLTextAreaElement {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
}

export function isSelectAllKey(
  e: Pick<KeyboardEvent, "metaKey" | "ctrlKey" | "altKey" | "key">,
): boolean {
  return (e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === "a";
}

export function applySelectAllInTextField(e: KeyboardEvent): boolean {
  if (!isSelectAllKey(e) || !isEditableTextField(e.target)) return false;
  e.preventDefault();
  e.target.select();
  return true;
}
