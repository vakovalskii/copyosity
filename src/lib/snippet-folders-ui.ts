const COLLAPSED_FOLDER_IDS_KEY = "snippetFolderCollapsedIds";

export function loadCollapsedSnippetFolderIds(): Set<number> {
  try {
    const raw = localStorage.getItem(COLLAPSED_FOLDER_IDS_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return new Set();
    const ids = parsed.filter((id): id is number => typeof id === "number" && Number.isInteger(id));
    return new Set(ids);
  } catch {
    return new Set();
  }
}

export function saveCollapsedSnippetFolderIds(collapsed: Set<number>): void {
  localStorage.setItem(COLLAPSED_FOLDER_IDS_KEY, JSON.stringify([...collapsed]));
}

export function isSnippetFolderExpanded(folderId: number, collapsed: Set<number>): boolean {
  return !collapsed.has(folderId);
}

export function pruneCollapsedSnippetFolderIds(
  collapsed: Set<number>,
  activeFolderIds: number[],
): Set<number> {
  const active = new Set(activeFolderIds);
  return new Set([...collapsed].filter((id) => active.has(id)));
}
