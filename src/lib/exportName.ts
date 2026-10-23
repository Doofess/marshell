export type ExportFormat = "markdown" | "json";

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
  return `${words || "conversation"}-${date}.${format === "markdown" ? "md" : "json"}`;
}
