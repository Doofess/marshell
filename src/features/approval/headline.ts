import type { ToolDetail } from "./types";

const MINUS = "−";

/** The host of a URL, or the raw text when it does not parse. */
function host(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}

/** The card's "what", as a verb phrase (docs/PLAN.md "The approve card"). */
export function headline(d: ToolDetail): string {
  switch (d.kind) {
    case "bash":
      return "Run a command";
    case "edit":
      return `Edit ${d.path} (+${d.add} ${MINUS}${d.del})`;
    case "write":
      return d.isNew ? `Create ${d.path}` : `Overwrite ${d.path}`;
    case "fetch":
      return `Fetch ${host(d.url)}`;
    case "mcp":
      return `Use ${d.server} · ${d.tool}`;
    case "question":
      return "Has a question";
  }
}

const cut = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

/** What a request was about, in a few words: the command's first line, the path, the host or the tool. */
export function subject(d: ToolDetail): string {
  switch (d.kind) {
    case "bash":
      return cut(d.command.split("\n")[0] ?? "", 40);
    case "edit":
    case "write":
      return d.path;
    case "fetch":
      return host(d.url);
    case "mcp":
      return d.tool;
    case "question":
      return "question";
  }
}

/** One line shown after a decision: "Allowed · npm test". */
export function receiptText(d: ToolDetail, verdict: "allowed" | "denied"): string {
  return `${verdict === "allowed" ? "Allowed" : "Denied"} · ${subject(d)}`;
}
