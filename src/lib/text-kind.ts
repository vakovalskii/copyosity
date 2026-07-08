/** Text kinds recognised for mono-font preview (card body + Quick Look). */
export const MONO_TEXT_KINDS = new Set([
  "JSON",
  "Shell",
  "Bash",
  "SQL",
  "HTML",
  "JavaScript",
  "TypeScript",
  "Python",
  "Rust",
]);

/** Heuristic kind label for copied text — drives mono vs system font in previews. */
export function detectTextKind(text: string | null): string {
  if (!text) return "Text";

  const sample = text.trim();
  const lower = sample.toLowerCase();

  if (/^(https?:\/\/|www\.)/.test(lower)) return "URL";
  if (
    sample.length < 10000 &&
    ((sample.startsWith("{") && sample.endsWith("}")) ||
      (sample.startsWith("[") && sample.endsWith("]")))
  ) {
    try {
      JSON.parse(sample);
      return "JSON";
    } catch {
      // fall through
    }
  }
  if (/^#!\/.*\b(bash|sh|zsh)\b/.test(lower)) return "Shell";
  if (
    /^(\$|#)\s+\S+/.test(sample) ||
    /\b(curl|git|npm|pnpm|yarn|brew|ssh|docker|kubectl)\b/.test(lower)
  ) {
    return "Bash";
  }
  if (/(^|\n)\s*(select|insert|update|delete|create table|alter table)\b/.test(lower)) return "SQL";
  if (/<[a-z][\s\S]*>/.test(lower)) return "HTML";
  if (/\b(function|const|let|import|export|=>)\b/.test(lower)) return "JavaScript";
  if (/\b(interface|type\s+\w+|implements|enum)\b/.test(lower)) return "TypeScript";
  if (/(^|\n)\s*(def |class |import |from .+ import )/.test(sample)) return "Python";
  if (/(^|\n)\s*(fn |let mut |impl |pub struct )/.test(sample)) return "Rust";

  return "Text";
}

/** True when a detected text kind should render in the monospace font. */
export function usesMonoPreview(textKind: string): boolean {
  return MONO_TEXT_KINDS.has(textKind);
}
