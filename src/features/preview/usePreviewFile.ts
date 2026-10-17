import { useEffect, useRef, useState } from "react";
import { IMAGE_CAP, TEXT_CAP, fetchContent, statFile, type Content } from "./previewApi";
import { decodeText, kindOf } from "./kinds";
import { START, next, type ReloadState } from "./reload";

/** What the pane shows for the file: one state at a time. */
export type FileState =
  | { phase: "loading" }
  | { phase: "text"; text: string }
  | { phase: "image"; url: string }
  | { phase: "tooLarge"; size: number; limit: number }
  | { phase: "binary" }
  | { phase: "pdf" }
  | { phase: "deleted"; /** What was shown before the file went; it stays on screen under the notice. */ last?: Shown }
  | { phase: "error" };

/** The states that carry content worth keeping on screen when the file disappears. */
export type Shown = Extract<FileState, { phase: "text" | "image" }>;

/** The state after a poll finds the file gone: the last text or image stays; anything else gives the plain message. */
export function afterDeleted(prev: FileState): FileState {
  if (prev.phase === "deleted") return prev;
  if (prev.phase === "text" || prev.phase === "image") return { phase: "deleted", last: prev };
  return { phase: "deleted" };
}

/** How often the open file is checked for a change. */
export const POLL_MS = 1000;
/** How long the "Updated" note stays. */
export const UPDATED_MS = 2000;

const IMAGE_TYPES: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml", bmp: "image/bmp", ico: "image/x-icon", avif: "image/avif" };
export const imageType = (path: string) => IMAGE_TYPES[path.slice(path.lastIndexOf(".") + 1).toLowerCase()] ?? "application/octet-stream";

/**
 * Loads a file and keeps it current: `stat` once a second while the document is visible, and a refetch only when the size or
 * the modification time moved (`next()` in reload.ts decides). A refetch replaces the content in place, so the scroll position
 * stays; `updated` is true for two seconds after one. The hook lives as long as the pane is open, so it stops with it.
 */
export function usePreviewFile(path: string): { file: FileState; updated: boolean } {
  const [file, setFile] = useState<FileState>({ phase: "loading" });
  const [updated, setUpdated] = useState(false);
  const loaded = useRef(false);

  useEffect(() => {
    let cancelled = false;
    let busy = false;
    let note: ReturnType<typeof setTimeout> | undefined;
    let state: ReloadState = START;
    loaded.current = false;
    setFile({ phase: "loading" });
    setUpdated(false);

    const kind = kindOf(path);
    if (kind === "pdf") {
      setFile({ phase: "pdf" });
      return;
    }
    const wire = kind === "image" ? "image" : "text";
    const cap = kind === "image" ? IMAGE_CAP : TEXT_CAP;

    const show = (content: Content) => {
      if (content.kind === "missing") {
        state = { seen: null, status: "deleted" };
        loaded.current = false;
        setFile(afterDeleted);
        return;
      }
      if (content.kind === "tooLarge") {
        loaded.current = false;
        setFile({ phase: "tooLarge", size: content.size, limit: content.limit });
        return;
      }
      if (wire === "image") {
        setFile({ phase: "image", url: URL.createObjectURL(new Blob([content.bytes], { type: imageType(path) })) });
      } else {
        const d = decodeText(content.bytes);
        setFile(d.kind === "text" ? { phase: "text", text: d.text } : { phase: "binary" });
      }
      if (loaded.current) {
        setUpdated(true);
        clearTimeout(note);
        note = setTimeout(() => setUpdated(false), UPDATED_MS);
      }
      loaded.current = true;
    };

    const tick = async (first: boolean) => {
      if (busy || cancelled) return;
      busy = true;
      try {
        const stat = await statFile(path);
        if (cancelled) return;
        const step = next(state, stat, cap);
        state = step.state;
        if (step.action === "refetch") {
          const content = await fetchContent(path, wire);
          if (!cancelled) show(content);
        } else if (step.action === "tooLarge" && stat.kind === "ok") {
          loaded.current = false;
          setFile({ phase: "tooLarge", size: stat.size, limit: cap });
        } else if (step.action === "deleted") {
          loaded.current = false;
          setFile(afterDeleted);
        }
      } catch {
        // A refused path or a core that is away: say so on the first look; later polls try again quietly.
        if (first && !cancelled) setFile({ phase: "error" });
      } finally {
        busy = false;
      }
    };

    void tick(true);
    const poll = () => {
      if (document.visibilityState === "visible") void tick(false);
    };
    const timer = setInterval(poll, POLL_MS);
    // Coming back to the window catches up at once instead of waiting for the next second.
    document.addEventListener("visibilitychange", poll);
    return () => {
      cancelled = true;
      clearInterval(timer);
      clearTimeout(note);
      document.removeEventListener("visibilitychange", poll);
    };
  }, [path]);

  // An image's object URL lives as long as it is shown, and is let go when it is replaced or the pane closes.
  const shown = file.phase === "deleted" ? file.last : file;
  const url = shown?.phase === "image" ? shown.url : undefined;
  useEffect(() => {
    if (!url) return;
    return () => URL.revokeObjectURL(url);
  }, [url]);

  return { file, updated };
}
