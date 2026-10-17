import { useEffect, useState } from "react";
import type { FileList } from "../../generated/FileList";
import type { QuickFiles } from "../palette/Palette";
import { terminalBuffer } from "../terminal/terminals";
import { seenPaths } from "./outputPaths";
import { displayPath, openPath } from "./quickOpen";
import { listFiles } from "./previewApi";

/** A fetched list is reused this long: opening the palette twice in a row should not walk the folder twice. */
const TTL_MS = 10_000;

type Entry = { at: number; list: Promise<FileList> };
const shared = new Map<string, Entry>();

/** The session's file list, from the core at most once per 10 s. Keyed by tab, because the root is only known once the core has answered. */
export function cachedFileList(tab: string, fetch: (tab: string) => Promise<FileList> = listFiles, now: () => number = Date.now, cache: Map<string, Entry> = shared): Promise<FileList> {
  const hit = cache.get(tab);
  if (hit && now() - hit.at < TTL_MS) return hit.list;
  const list = fetch(tab);
  cache.set(tab, { at: now(), list });
  // A failed request is not remembered.
  list.catch(() => cache.get(tab)?.list === list && cache.delete(tab));
  return list;
}

/** The last part of a folder path, for "Files in <folder>". */
export const folderName = (root: string): string => root.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || root;

export type QuickState = { files: QuickFiles; pathOf: (display: string) => string };

/**
 * The palette's quick open for one session, from the state of the folder's list (`"loading"`, `"error"`, or the list) and the absolute
 * paths seen in the output. The seen paths show at once: as given until the list says where the root is, then relative to it.
 */
export function buildQuickFiles(list: FileList | "loading" | "error", seenAbs: string[]): QuickState {
  if (typeof list === "string") {
    const absOf = new Map(seenAbs.map((abs) => [displayPath(abs, ""), abs]));
    return { files: { seen: [...absOf.keys()], list: [], folder: "", status: list }, pathOf: (display) => absOf.get(display) ?? display };
  }
  const absOf = new Map(seenAbs.map((abs) => [displayPath(abs, list.root), abs]));
  return {
    files: { seen: [...absOf.keys()], list: list.files, folder: folderName(list.root), truncated: list.truncated, status: "ready" },
    pathOf: (display) => absOf.get(display) ?? openPath(display, list.root),
  };
}

/**
 * What the palette needs for quick open on a session: its files and the paths seen in its output, as display paths, and a way to turn a
 * chosen one back into the path to open. The list has an explicit state (loading, error, ready) and the seen paths do not wait for it.
 * `null` only when there is no session.
 */
export function useQuickFiles(tab: string | null): QuickState | null {
  const [list, setList] = useState<FileList | "loading" | "error">("loading");
  const [seenAbs, setSeenAbs] = useState<string[]>([]);
  useEffect(() => {
    if (tab === null) return;
    let live = true;
    setList("loading");
    setSeenAbs([]);
    cachedFileList(tab).then(
      (l) => live && setList(l),
      () => live && setList("error"),
    );
    const buffer = terminalBuffer(tab);
    if (buffer) {
      seenPaths(tab, buffer).then(
        (paths) => live && setSeenAbs(paths),
        () => {},
      );
    }
    return () => {
      live = false;
    };
  }, [tab]);
  return tab === null ? null : buildQuickFiles(list, seenAbs);
}
