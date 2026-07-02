import type { ConfirmMessageBody, ConfirmMessagePart } from "./confirm";
import type { HistoryCounts } from "./types";

export type ClearHistoryAction = "unpinned" | "all";

const NBSP = "\u00A0";

function itemsWord(count: number): string {
  return count === 1 ? "item" : "items";
}

function pinnedWord(count: number): string {
  return count === 1 ? "pinned item" : "pinned items";
}

/** Number + following words stay on one line when the dialog wraps. */
function withCount(count: number, trailing: string): ConfirmMessagePart[] {
  return [{ text: count.toLocaleString(), emph: true }, { text: `${NBSP}${trailing}` }];
}

export function clearUnpinnedConfirmBody(counts: HistoryCounts): ConfirmMessageBody {
  const count = counts.unpinned;
  return {
    primary: withCount(count, `unpinned ${itemsWord(count)} will be permanently deleted.`),
    secondary: [{ text: "Pinned items will be kept." }],
  };
}

export function clearAllConfirmBody(counts: HistoryCounts): ConfirmMessageBody {
  const { total, pinned } = counts;

  if (pinned === 0) {
    return {
      primary: withCount(total, `${itemsWord(total)} will be permanently deleted.`),
    };
  }

  return {
    primary: withCount(total, `${itemsWord(total)} will be permanently deleted.`),
    secondary: [{ text: "This includes " }, ...withCount(pinned, `${pinnedWord(pinned)}.`)],
  };
}
