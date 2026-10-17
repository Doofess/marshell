export type PreviewKind = "code" | "markdown" | "html" | "image" | "pdf" | "text";

/** The Shiki language id of each extension the preview calls code (or markdown or html). Anything else is plain text. */
const LANGUAGE: Record<string, string> = {
  ts: "typescript", tsx: "tsx", mts: "typescript", cts: "typescript", js: "javascript", jsx: "jsx", mjs: "javascript", cjs: "javascript",
  json: "json", jsonc: "jsonc", json5: "json5", yaml: "yaml", yml: "yaml", toml: "toml", xml: "xml", css: "css", scss: "scss", less: "less",
  rs: "rust", go: "go", py: "python", rb: "ruby", php: "php", java: "java", kt: "kotlin", swift: "swift", c: "c", h: "c", cpp: "cpp",
  cc: "cpp", hpp: "cpp", cs: "csharp", lua: "lua", sh: "bash", bash: "bash", zsh: "bash", ps1: "powershell", psm1: "powershell",
  bat: "bat", cmd: "bat", sql: "sql", graphql: "graphql", vue: "vue", svelte: "svelte", dockerfile: "docker", diff: "diff", patch: "diff",
  ini: "ini", md: "markdown", markdown: "markdown", mdx: "markdown", html: "html", htm: "html",
};

const IMAGE = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"]);

/** The part of the last path segment after its last dot, lower-cased; "" for none ("Makefile", ".gitignore"). */
function extensionOf(path: string): string {
  const name = splitPath(path).name;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** What the preview shows a file as, from its extension alone. An unknown extension is text; the bytes decide later whether it really is. */
export function kindOf(path: string): PreviewKind {
  const ext = extensionOf(path);
  if (ext === "pdf") return "pdf";
  if (IMAGE.has(ext)) return "image";
  if (ext === "md" || ext === "markdown" || ext === "mdx") return "markdown";
  if (ext === "html" || ext === "htm") return "html";
  return LANGUAGE[ext] ? "code" : "text";
}

/** The Shiki language of a file, or undefined for plain text. */
export function languageOf(path: string): string | undefined {
  return LANGUAGE[extensionOf(path)];
}

/** The file name and its folder, for either kind of separator. The folder is empty for a bare name. */
export function splitPath(path: string): { name: string; folder: string } {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut < 0 ? { name: path, folder: "" } : { name: path.slice(cut + 1), folder: path.slice(0, cut) };
}

export type Decoded = { kind: "text"; text: string } | { kind: "binary" };

const SNIFF_BYTES = 8192;

/** Bytes as UTF-8 text, or "binary" when a NUL byte sits in the first 8 KB or the bytes are not valid UTF-8. */
export function decodeText(bytes: ArrayBuffer): Decoded {
  const head = new Uint8Array(bytes, 0, Math.min(SNIFF_BYTES, bytes.byteLength));
  if (head.includes(0)) return { kind: "binary" };
  try {
    return { kind: "text", text: new TextDecoder("utf-8", { fatal: true }).decode(bytes) };
  } catch {
    return { kind: "binary" };
  }
}
