export type LinkTarget =
  | { kind: "anchor"; id: string }
  | { kind: "local"; path: string; line?: number }
  | { kind: "web"; url: string }
  | { kind: "none" };

const NONE: LinkTarget = { kind: "none" };
const SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;
const DRIVE = /^[a-zA-Z]:[\\/]/;
/** `#L12`, `#L12-L20`, `#L12C3`: a GitHub-style line reference. Only the first line is used. */
const LINE = /^L(\d+)(?:C\d+)?(?:-L?\d+(?:C\d+)?)?$/i;
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f]/;

const decode = (s: string): string => {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
};

/** A path on a network share or a device (`\\server\share`, `//server/x`, `\\?\C:\`): never touched, because opening one can leak credentials. */
const isNetworkPath = (p: string) => /^[\\/]{2}/.test(p);

/**
 * What a link in a document means. Only http(s) is a web link; `#id` is an anchor; a path with no scheme is a local file, resolved
 * against the document's folder. Every other scheme (javascript:, mailto:, file:, data:, blob:…) and any network path is `none`.
 */
export function classifyLink(href: string, docPath: string): LinkTarget {
  const raw = href.trim();
  if (!raw) return NONE;
  if (raw.startsWith("#")) {
    const id = decode(raw.slice(1));
    return id && !CONTROL.test(id) ? { kind: "anchor", id } : NONE;
  }
  if (SCHEME.test(raw) && !DRIVE.test(raw)) {
    if (!/^https?:/i.test(raw)) return NONE;
    try {
      const url = new URL(raw);
      return url.hostname ? { kind: "web", url: url.href } : NONE;
    } catch {
      return NONE;
    }
  }
  const hash = raw.indexOf("#");
  const fragment = hash < 0 ? "" : raw.slice(hash + 1);
  const target = decode(hash < 0 ? raw : raw.slice(0, hash));
  if (!target || CONTROL.test(target) || isNetworkPath(target)) return NONE;
  const path = resolve(target, docPath);
  if (!path) return NONE;
  const line = LINE.exec(fragment)?.[1];
  return line && Number(line) >= 1 ? { kind: "local", path, line: Number(line) } : { kind: "local", path };
}

/** The path joined to the document's folder and cleaned of `.` and `..`, in the document's own separator style. Null if it has none to use. */
function resolve(target: string, docPath: string): string | null {
  const sep = docPath.includes("\\") ? "\\" : "/";
  const split = (p: string) => p.split(/[\\/]+/).filter((s) => s !== "");
  const doc = split(docPath);
  const drive = /^[a-zA-Z]:$/.test(doc[0] ?? "") ? doc[0]! : "";
  let rooted: string;
  let parts: string[];
  if (DRIVE.test(target)) {
    const t = split(target);
    rooted = t[0]!;
    parts = t.slice(1);
  } else if (/^[\\/]/.test(target)) {
    rooted = drive;
    parts = split(target);
  } else {
    rooted = drive;
    parts = [...doc.slice(drive ? 1 : 0, -1), ...split(target)];
  }
  const out: string[] = [];
  for (const p of parts) {
    if (p === ".") continue;
    if (p === "..") out.pop();
    else out.push(p);
  }
  if (out.length === 0) return null;
  return `${rooted}${sep}${out.join(sep)}`;
}
