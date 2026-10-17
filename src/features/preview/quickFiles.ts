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

/**
 * What the palette needs for quick open on a session: its files and the paths seen in its output, as display paths, and a way to turn a
 * chosen one back into the path to open. `null` until both have arrived, so nothing is shown while loading.
 */
export function useQuickFiles(tab: string | null): { files: QuickFiles; pathOf: (display: string) => string } | null {
  const [loaded, setLoaded] = useState<{ files: QuickFiles; pathOf: (display: string) => string } | null>(null);
  useEffect(() => {
    if (tab === null) return;
    let live = true;
    void (async () => {
      const list = await cachedFileList(tab);
      const buffer = terminalBuffer(tab);
      const seenAbs = buffer ? await seenPaths(tab, buffer).catch(() => []) : [];
      const absOf = new Map(seenAbs.map((abs) => [displayPath(abs, list.root), abs]));
      const pathOf = (display: string) => absOf.get(display) ?? openPath(display, list.root);
      if (live) setLoaded({ files: { seen: [...absOf.keys()], list: list.files, folder: folderName(list.root), truncated: list.truncated }, pathOf });
    })().catch(() => {});
    return () => {
      live = false;
    };
  }, [tab]);
  return loaded;
}
