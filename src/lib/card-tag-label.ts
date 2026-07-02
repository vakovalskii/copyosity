/**
 * Display-only tag labels for overlay UI (toolbar chips + card footer).
 *
 * DB keeps canonical tag strings from AI tagging; UI groups synonyms under one label
 * (e.g. `javascript` and `js` → chip `js`). Filtering matches any DB variant.
 *
 * Curated manually from common industry shorthand (k8s, i18n, a11y, js, ts) — extend only
 * when the short form is unambiguous in a clipboard-tag context.
 */
const CARD_TAG_ABBREVIATIONS: Readonly<Record<string, string>> = {
  accessibility: "a11y",
  application: "app",
  authentication: "auth",
  configuration: "config",
  database: "db",
  development: "dev",
  documentation: "docs",
  environment: "env",
  infrastructure: "infra",
  internationalization: "i18n",
  javascript: "js",
  kubernetes: "k8s",
  performance: "perf",
  production: "prod",
  repository: "repo",
  typescript: "ts",
};

/** Canonical UI label for a DB tag (toolbar chip key + card footer text). */
export function cardTagDisplayLabel(tag: string): string {
  const key = tag.trim().toLowerCase();
  return CARD_TAG_ABBREVIATIONS[key] ?? tag;
}

/** @deprecated Use {@link cardTagDisplayLabel}. */
export const formatCardTagLabel = cardTagDisplayLabel;

/** DB tag strings that share a UI label (e.g. `js` → [`javascript`, `js`]). */
export function cardTagDbVariants(displayLabel: string): string[] {
  const label = displayLabel.trim().toLowerCase();
  const variants = new Set<string>([label]);

  for (const [longForm, shortForm] of Object.entries(CARD_TAG_ABBREVIATIONS)) {
    if (shortForm === label) {
      variants.add(longForm);
    }
  }

  return [...variants].toSorted((a, b) => a.localeCompare(b));
}

/** True when two DB tags render as the same UI label. */
export function cardTagsShareDisplayLabel(a: string, b: string): boolean {
  return cardTagDisplayLabel(a) === cardTagDisplayLabel(b);
}

/** 1–2 visible tags: only labels longer than this may ellipsize. */
export const CARD_TAG_TRUNCATE_LABEL_MIN = 12;

/** 3 visible tags: total label length budget before any ellipsize. */
export const CARD_TAG_THREE_LABEL_BUDGET = 22;

/** Labels this short (inclusive) never ellipsize, even in a 3-tag row. */
export const CARD_TAG_NEVER_TRUNCATE_MAX_LEN = 7;

export function cardTagShouldTruncate(label: string): boolean {
  return label.length > CARD_TAG_TRUNCATE_LABEL_MIN;
}

/**
 * Approximate visible length once ellipsis is applied — used only to decide which
 * chips get `.entry-tag-truncates`, not for pixel-perfect layout.
 */
function truncatedLabelBudgetLength(labelLength: number): number {
  return Math.max(4, labelLength - 4);
}

/**
 * Three-tag row: when label lengths sum to more than {@link CARD_TAG_THREE_LABEL_BUDGET},
 * mark chips for ellipsis starting with the longest label(s). Two tags tied for the
 * current max length are truncated together; otherwise one chip at a time. Tags ≤7
 * chars are never marked.
 */
export function cardTagThreeTruncateFlags(labels: readonly string[]): boolean[] {
  if (labels.length !== 3) {
    throw new Error(`expected 3 labels, got ${labels.length}`);
  }

  const lengths = labels.map((label) => label.length);
  let effectiveTotal = lengths.reduce((sum, length) => sum + length, 0);
  if (effectiveTotal <= CARD_TAG_THREE_LABEL_BUDGET) {
    return [false, false, false];
  }

  const flags = [false, false, false];

  while (effectiveTotal > CARD_TAG_THREE_LABEL_BUDGET) {
    const candidates = lengths
      .map((length, index) => ({ index, length }))
      .filter(({ index, length }) => !flags[index] && length > CARD_TAG_NEVER_TRUNCATE_MAX_LEN);

    if (candidates.length === 0) break;

    const maxLength = Math.max(...candidates.map((candidate) => candidate.length));
    const atMax = candidates.filter((candidate) => candidate.length === maxLength);
    const toTruncate = atMax.length === 2 ? atMax : [atMax[0]!];

    for (const { index, length } of toTruncate) {
      flags[index] = true;
      effectiveTotal -= length - truncatedLabelBudgetLength(length);
    }
  }

  return flags;
}

export function cardTagFooterTruncateFlags(labels: readonly string[]): boolean[] {
  if (labels.length === 3) {
    return cardTagThreeTruncateFlags(labels);
  }
  return labels.map((label) => cardTagShouldTruncate(label));
}

/** Full tag name for tooltip / screen readers when the label is abbreviated or may clip. */
export function cardTagTitle(tag: string, label: string, truncates: boolean): string | undefined {
  if (label !== tag || truncates) return tag;
  return undefined;
}
