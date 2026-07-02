/** Visible copy for resolved app names (buttons, list rows, success notices). */
function formatAppNameForLabel(appName: string): string {
  const trimmed = appName.trim();
  if (!trimmed) return trimmed;
  return trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
}

/** Overlay — visible button / status copy (section context implies clipboard). */
export function excludeFromHistoryLabel(appName: string): string {
  return `Exclude ${formatAppNameForLabel(appName)} from history`;
}

/** Settings active-app row — context from `ExcludableAppCandidate.source`. */
export function excludableCandidateMetaLabel(source: "remembered" | "frontmost"): string {
  return source === "frontmost" ? "Active app" : "Recent app";
}

/** Settings list row — compact visible label. */
export function excludeListAddLabel(): string {
  return "Add";
}

export function excludeListRemoveLabel(): string {
  return "Remove";
}

/** Settings row actions — full meaning for aria-label / title. */
export function excludeFromClipboardHistoryAriaLabel(appName: string): string {
  return `Exclude ${formatAppNameForLabel(appName)} from clipboard history`;
}

export function allowInClipboardHistoryAriaLabel(appName: string): string {
  return `Allow ${formatAppNameForLabel(appName)} in clipboard history`;
}

/** Overlay status when an app is already excluded from capture. */
export function alreadyExcludedFromHistoryLabel(appName: string): string {
  return `${formatAppNameForLabel(appName)} excluded from history`;
}

export function excludedFromHistoryNotice(appName: string): string {
  return `${formatAppNameForLabel(appName)} excluded from history`;
}

/** Settings — app is already on the exclusion list. */
export function alreadyExcludedFromHistoryNotice(appName: string): string {
  return `${formatAppNameForLabel(appName)} is already excluded`;
}

export function allowedInHistoryNotice(appName: string): string {
  return `${formatAppNameForLabel(appName)} allowed in history`;
}

/** Settings — label for the native app picker row (keep in sync with Settings UI). */
export const chooseApplicationActionLabel = "Choose Application…";

/** Settings — app name could not be resolved (HIG-style inline warning). */
export function appNotFoundNotice(appName: string): string {
  const name = appName.trim();
  return `No app named “${name}” was found. Use ${chooseApplicationActionLabel}, or enter the installed app name.`;
}

/** Settings — generic add-by-name failure with the entered app name. */
export function couldNotAddExcludedAppNotice(appName: string): string {
  const name = appName.trim();
  return `Could not add ${name}. Try again.`;
}

/** Settings — add via native picker failed without a resolved app name. */
export function couldNotAddSelectedAppNotice(): string {
  return "Could not add the selected app. Try again.";
}

export function invokeErrorMessage(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  if (err && typeof err === "object") {
    if ("message" in err && typeof (err as { message: unknown }).message === "string") {
      return (err as { message: string }).message;
    }
    if ("error" in err && typeof (err as { error: unknown }).error === "string") {
      return (err as { error: string }).error;
    }
  }
  return "";
}

export function isAppNotFoundError(err: unknown): boolean {
  return invokeErrorMessage(err).includes("app_not_found:");
}

/** Settings candidate row when list and candidate are out of sync. */
export function alreadyExcludedListMetaLabel(): string {
  return "Already excluded";
}
