import type { TreeRow } from "./treeModel";

export type IconKind = "folder" | "folder-open" | "code" | "markdown" | "text" | "image" | "pdf" | "web" | "data" | "config" | "archive" | "other" | "link";

const words = (s: string) => new Set(s.split(" "));
const BY_EXT: [IconKind, Set<string>][] = [
  ["code", words("ts tsx js jsx mjs cjs rs py go java kt swift c h cc cpp hpp cs rb php lua sh bash zsh ps1 bat cmd css scss sass less vue svelte sql")],
  ["markdown", words("md mdx markdown")],
  ["text", words("txt log rst")],
  ["image", words("png jpg jpeg gif webp svg ico bmp avif tif tiff")],
  ["pdf", words("pdf")],
  ["web", words("html htm xhtml")],
  ["data", words("json jsonc json5 ndjson csv tsv xml parquet")],
  ["config", words("toml yaml yml ini cfg conf env lock editorconfig properties")],
  ["archive", words("zip tar gz tgz bz2 xz 7z rar")],
];

/** Which glyph a row wears: by its kind, then by its extension. A dotfile with no extension of its own (".gitignore") is a config. */
export function iconKind(name: string, kind: TreeRow["kind"], expanded: boolean): IconKind {
  if (kind === "folder") return expanded ? "folder-open" : "folder";
  if (kind === "link") return "link";
  if (kind !== "file") return "other";
  const dot = name.lastIndexOf(".");
  if (dot === 0 && name.length > 1) return "config";
  if (dot <= 0) return "other";
  const ext = name.slice(dot + 1).toLowerCase();
  return BY_EXT.find(([, set]) => set.has(ext))?.[0] ?? "other";
}

const PAGE = "M3.5 1.5h4.5l3 3v8h-7.5Z M8 1.5v3h3";

/** A 14 px line glyph in the current colour. Decoration: the row's name says what it is. */
export function FileIcon({ kind }: { kind: IconKind }) {
  return (
    <svg className="file-tree__icon" data-kind={kind} width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {kind === "folder" && <path d="M1.5 3.5h3.8l1.2 1.4h5v6.6h-10Z" />}
      {kind === "folder-open" && (
        <>
          <path d="M1.5 11.5v-8h3.8l1.2 1.4h4v1.6" />
          <path d="M1.5 11.5 3.2 6.5h9.3l-1.7 5Z" />
        </>
      )}
      {kind === "code" && (
        <>
          <path d={PAGE} />
          <path d="M6 7 5 8.2 6 9.4M8 7l1 1.2L8 9.4" />
        </>
      )}
      {kind === "markdown" && (
        <>
          <path d={PAGE} />
          <path d="M5 11V8l1 1.2L7 8v3" />
        </>
      )}
      {kind === "text" && (
        <>
          <path d={PAGE} />
          <path d="M5.5 7h3M5.5 9h3M5.5 11h1.5" />
        </>
      )}
      {kind === "image" && (
        <>
          <rect x="1.5" y="2.5" width="11" height="9" rx="1" />
          <circle cx="5" cy="5.5" r="1" />
          <path d="m2 11 3.5-3 2.5 2 2-1.5 2.5 2.5" />
        </>
      )}
      {kind === "pdf" && (
        <>
          <path d={PAGE} />
          <path d="M5 8.5h4v2.5H5Z" />
        </>
      )}
      {kind === "web" && (
        <>
          <circle cx="7" cy="7" r="5.5" />
          <path d="M1.5 7h11M7 1.5c1.7 1.5 2.5 3.4 2.5 5.5S8.7 11 7 12.5C5.3 11 4.5 9.1 4.5 7S5.3 3 7 1.5Z" />
        </>
      )}
      {kind === "data" && (
        <>
          <path d={PAGE} />
          <path d="M5.5 7.5h3M5.5 9.5h3M7 6.5v4" />
        </>
      )}
      {kind === "config" && (
        <>
          <path d="M2 4h10M2 7h10M2 10h10" />
          <circle cx="5" cy="4" r="1.2" />
          <circle cx="9" cy="7" r="1.2" />
          <circle cx="6" cy="10" r="1.2" />
        </>
      )}
      {kind === "archive" && (
        <>
          <path d={PAGE} />
          <path d="M7 6v1M7 8v1M7 10v1.5" />
        </>
      )}
      {kind === "other" && <path d={PAGE} />}
      {kind === "link" && (
        <>
          <path d="M6 8a2.4 2.4 0 0 0 3.4 0l2-2a2.4 2.4 0 0 0-3.4-3.4l-.6.6" />
          <path d="M8 6a2.4 2.4 0 0 0-3.4 0l-2 2a2.4 2.4 0 0 0 3.4 3.4l.6-.6" />
        </>
      )}
    </svg>
  );
}
