import { useCallback, useEffect, useRef, useState } from "react";
import { IMAGE_CAP, PDF_CAP, TEXT_CAP, fetchContent, statFile, type Content } from "./previewApi";
import { decodeFile, type Encoding, type Eol } from "../editor/textFile";
import { kindOf } from "./kinds";
import { START, next, type ReadOnlyReason, type ReloadState } from "./reload";

/** What the pane shows for the file: one state at a time. */
export type FileState =
  | { phase: "loading" }
  | { phase: "text"; text: string; encoding: Encoding; eol: Eol; hash: string; readOnly: ReadOnlyReason | null }
  | { phase: "image"; url: string }
  | { phase: "tooLarge"; size: number; limit: number }
  | { phase: "binary" }
  | { phase: "pdf"; bytes: ArrayBuffer; /** Counts refetches from 0, so the view reloads while keeping its scroll. */ version: number }
  | { phase: "deleted"; /** What was shown before the file went; it stays on screen under the notice. */ last?: Shown }
  | { phase: "error" };

/** The states that carry content worth keeping on screen when the file disappears. */
export type Shown = Extract<FileState, { phase: "text" | "image" | "pdf" }>;

/** The state after a poll finds the file gone: the last text or image stays; anything else gives the plain message. */
export function afterDeleted(prev: FileState): FileState {
  if (prev.phase === "deleted") return prev;
  if (prev.phase === "text" || prev.phase === "image" || prev.phase === "pdf") return { phase: "deleted", last: prev };
  return { phase: "deleted" };
}

const LOADING: FileState = { phase: "loading" };

/**
 * What the pane shows for `path`, from a state read for `of`. The render where the tab goes to another file comes before the
 * loader starts on it, so the state is still the last file's then: it shows as loading, and that file's pages or text never
 * appear under the new entry.
 */
export function shownFile(file: FileState, of: string, path: string): FileState {
  return of === path ? file : LOADING;
}

/** How often the open file is checked for a change. */
export const POLL_MS = 1000;
/** How long the "Updated" note stays. */
export const UPDATED_MS = 2000;

/**
 * The schedule of a file's checks: one at once unless paused, then one a second while visible and not paused. `resume` checks once
 * (the first look, if a paused file has had none). A paused file makes no request at all.
 */
export function createPoller(o: { tick: (first: boolean) => void; isPaused: () => boolean; visible: () => boolean }) {
  let looked = false;
  let timer: ReturnType<typeof setInterval> | undefined;
  const look = () => {
    const first = !looked;
    looked = true;
    o.tick(first);
  };
  const poll = () => {
    if (!o.isPaused() && o.visible()) look();
  };
  return {
    start() {
      if (!o.isPaused()) look();
      timer = setInterval(poll, POLL_MS);
    },
    poll,
    resume() {
      if (!o.isPaused()) look();
    },
    stop() {
      clearInterval(timer);
    },
  };
}

const IMAGE_TYPES: Record<string, string> = { png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml", bmp: "image/bmp", ico: "image/x-icon", avif: "image/avif" };
export const imageType = (path: string) => IMAGE_TYPES[path.slice(path.lastIndexOf(".") + 1).toLowerCase()] ?? "application/octet-stream";

/**
 * Loads a file and keeps it current: `stat` once a second while the document is visible, and a refetch only when the size or
 * the modification time moved (`next()` in reload.ts decides). A refetch replaces the content in place, so the scroll position
 * stays; `updated` is true for two seconds after one. The hook lives as long as the pane is open, so it stops with it.
 */
export function usePreviewFile(path: string, opts: { paused?: boolean } = {}): { file: FileState; updated: boolean; refetch: () => void } {
  const paused = opts.paused === true;
  const [file, setFile] = useState<FileState>({ phase: "loading" });
  // The path `file` was read for; the effect below moves it with the path.
  const [fileOf, setFileOf] = useState(path);
  const [updated, setUpdated] = useState(false);
  const loaded = useRef(false);
  const pausedRef = useRef(paused);
  pausedRef.current = paused;
  const poller = useRef<ReturnType<typeof createPoller> | null>(null);
  // Reads the file again whatever its stat says; the running effect sets it.
  const force = useRef<() => void>(() => {});
  const refetch = useCallback(() => force.current(), []);
  const wasPaused = useRef(paused);
  // Resuming checks once at once; the interval carries on from there.
  useEffect(() => {
    if (wasPaused.current && !paused) poller.current?.resume();
    wasPaused.current = paused;
  }, [paused]);

  useEffect(() => {
    let cancelled = false;
    let busy = false;
    // A forced read came while a check was running: that check may have read the file before the change, so one more follows.
    let again = false;
    let note: ReturnType<typeof setTimeout> | undefined;
    let state: ReloadState = START;
    loaded.current = false;
    setFile({ phase: "loading" });
    setFileOf(path);
    setUpdated(false);

    const kind = kindOf(path);
    const wire = kind === "pdf" ? "pdf" : kind === "image" ? "image" : "text";
    const cap = kind === "pdf" ? PDF_CAP : kind === "image" ? IMAGE_CAP : TEXT_CAP;
    let version = 0;

    const show = (content: Content, readOnly: ReadOnlyReason | null) => {
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
      if (wire === "pdf") {
        setFile({ phase: "pdf", bytes: content.bytes, version: version++ });
      } else if (wire === "image") {
        setFile({ phase: "image", url: URL.createObjectURL(new Blob([content.bytes], { type: imageType(path) })) });
      } else {
        const d = decodeFile(content.bytes);
        setFile(d.kind === "text" ? { phase: "text", text: d.text, encoding: d.encoding, eol: d.eol, hash: content.hash, readOnly } : { phase: "binary" });
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
        // Every stat tells whether the file is read-only now, which a change of attribute alone does not move the size or time for.
        if (stat.kind === "ok") setFile((f) => (f.phase === "text" && f.readOnly !== stat.readOnly ? { ...f, readOnly: stat.readOnly } : f));
        const step = next(state, stat, cap);
        state = step.state;
        if (step.action === "refetch") {
          const content = await fetchContent(path, wire);
          if (!cancelled) show(content, stat.kind === "ok" ? stat.readOnly : null);
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
        if (again) {
          again = false;
          state = START;
          void tick(false);
        }
      }
    };
    force.current = () => {
      if (busy) {
        again = true;
        return;
      }
      // The next check reads the file in full; a paused file does it when it resumes.
      state = START;
      if (!pausedRef.current) void tick(false);
    };

    const schedule = createPoller({ tick: (first) => void tick(first), isPaused: () => pausedRef.current, visible: () => document.visibilityState === "visible" });
    poller.current = schedule;
    schedule.start();
    // Coming back to the window catches up at once instead of waiting for the next second.
    document.addEventListener("visibilitychange", schedule.poll);
    return () => {
      cancelled = true;
      schedule.stop();
      if (poller.current === schedule) poller.current = null;
      clearTimeout(note);
      document.removeEventListener("visibilitychange", schedule.poll);
    };
  }, [path]);

  // An image's object URL lives as long as it is shown, and is let go when it is replaced or the pane closes.
  const shown = file.phase === "deleted" ? file.last : file;
  const url = shown?.phase === "image" ? shown.url : undefined;
  useEffect(() => {
    if (!url) return;
    return () => URL.revokeObjectURL(url);
  }, [url]);

  return { file: shownFile(file, fileOf, path), updated: updated && fileOf === path, refetch };
}
