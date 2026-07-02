/** Custom-collections row grows and scrolls when pills or the add field are present. */
export function isCollectionsScrollable(collectionCount: number, showAdd: boolean): boolean {
  return collectionCount > 0 || showAdd;
}

/** History scope is active (not Starred, not a named collection). */
export function isHistoryCollectionActive(activeId: number | null, activePinned: boolean): boolean {
  return activeId === null && !activePinned;
}

/** Custom collection pill pressed state. */
export function isCustomCollectionActive(
  collectionId: number,
  activeId: number | null,
  activePinned: boolean,
): boolean {
  return activeId === collectionId && !activePinned;
}
