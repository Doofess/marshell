export type ExportFormat = "markdown" | "json" | "asciicast";

const EXTENSION: Record<ExportFormat, string> = { markdown: "md", json: "json", asciicast: "cast" };

const MAX_WORDS = 40;

/** The file name an exported conversation is offered under: `fix-the-api-2026-10-23.md`. Lower-case ASCII
 *  letters and digits from the title (any other run becomes one hyphen), cut to 40, then the local date.
 *  The shell accepts only this shape, so a title can never put a path or an odd extension in the name. */
export function exportFileName(title: string, format: ExportFormat, now: Date): string {
  const words = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, MAX_WORDS)
    .replace(/-+$/, "");
  const two = (n: number) => String(n).padStart(2, "0");
  const date = `${String(now.getFullYear()).padStart(4, "0")}-${two(now.getMonth() + 1)}-${two(now.getDate())}`;
  return `${words || "conversation"}-${date}.${EXTENSION[format]}`;
}

/** The title that names the saved file. Without redaction it is the conversation's own title. With it, the title is read from the
 *  redacted export itself (the first `# ` line of markdown, the `title` of JSON or of an asciicast's header line), so a secret or a personal path in the title never
 *  reaches the file name; when the export has none, it is empty and the name falls back to "conversation". */
export function exportTitle(text: string, format: ExportFormat, title: string, redact: boolean): string {
  if (!redact) return title;
  if (format === "markdown") {
    const first = text.split(/\r?\n/, 1)[0] ?? "";
    return first.startsWith("# ") ? first.slice(2) : "";
  }
  try {
    // JSON is one document; an asciicast's header is its first line.
    const t = (JSON.parse(format === "asciicast" ? (text.split(/\r?\n/, 1)[0] ?? "") : text) as { title?: unknown }).title;
    return typeof t === "string" ? t : "";
  } catch {
    return "";
  }
}
