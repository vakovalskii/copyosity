/** Whether a lazy full-image fetch result should update Quick Look state. */
export function shouldApplyFullImageResult(
  requestSeq: number,
  currentSeq: number,
  cancelled: boolean,
): boolean {
  return !cancelled && requestSeq === currentSeq;
}

export function resolveFullImagePayload(imageData: string | null | undefined): string | null {
  return imageData ?? null;
}

export function initialQuickLookImageTab(): "image" | "text" {
  return "image";
}
