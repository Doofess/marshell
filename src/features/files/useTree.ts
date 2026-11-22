import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { TreeAsk } from "../../generated/TreeAsk";
import type { TreeDir } from "../../generated/TreeDir";
import type { TreeReply } from "../../generated/TreeReply";
import type { TreeState } from "../../generated/TreeState";
import { ancestors, type Listing, openFolders, type TreeRow, visibleRows } from "./treeModel";

export const TREE_POLL_MS = 2000;
export const TREE_FULL_MS = 30000;
/** The folders in one request: the core refuses more. */
const ASKS_PER_REQUEST = 64;
/** The open folders kept per session. */
const MAX_SAVED = 500;
const SAVE_DEBOUNCE_MS = 500;
/** A round loads folders level by level; this bounds a loop that makes no progress. */
const MAX_PASSES = 50;

/** Where the tree's folders come from: the core's tree route for the session. */
export type TreeSource = { list(asks: TreeAsk[]): Promise<TreeReply | "no-folder"> };

/** Everything the tree knows about one session. */
export type TreeData = {
  session: string;
  /** The project folder the listings are relative to. Null until the first reply, or the saved one. */
  root: string | null;
  noFolder: boolean;
  listings: ReadonlyMap<string, Listing>;
  expanded: ReadonlySet<string>;
};

export function initialData(session: string, saved: TreeState | undefined): TreeData {
  return { session, root: saved?.root ?? null, noFolder: false, listings: new Map(), expanded: new Set((saved?.expanded ?? []).slice(0, MAX_SAVED)) };
}

/**
 * The folders to ask for: the root and every open folder that is drawn, in row order, with the ones never listed first so
 * a folder the user just opened is the first to load. Each carries the stamp of its last listing unless `full`. Cut into
 * requests of at most 64.
 */
export function nextAsks(listings: ReadonlyMap<string, Listing>, rows: readonly TreeRow[], full: boolean): TreeAsk[][] {
  const dirs = ["", ...openFolders(rows)];
  const ordered = [...dirs.filter((d) => !listings.has(d)), ...dirs.filter((d) => listings.has(d))];
  const asks = ordered.map((dir): TreeAsk => {
    const l = listings.get(dir);
    return { dir, stamp: full || !l || l.state !== "listed" ? null : l.stamp };
  });
  const out: TreeAsk[][] = [];
  for (let i = 0; i < asks.length; i += ASKS_PER_REQUEST) out.push(asks.slice(i, i + ASKS_PER_REQUEST));
  return out;
}

export type ScheduleInput = {
  inFlight: boolean;
  drawn: boolean;
  hasSource: boolean;
  visible: boolean;
  focused: boolean;
  now: number;
  lastAsk: number | null;
  lastFull: number | null;
  /** An expand, a reveal or a new source wants an answer now. */
  urgent: boolean;
  /** The window got the focus back: read everything afresh. */
  refull: boolean;
};

/**
 * What to ask now. Never while a request is out (one at a time). The first ask goes as soon as the tree is drawn; after it
 * the timed asks need a visible, focused window. A `full` ask carries no stamps.
 */
export function schedule(s: ScheduleInput): "none" | "poll" | "full" {
  if (s.inFlight || !s.drawn || !s.hasSource) return "none";
  if (s.lastAsk === null) return "full";
  const active = s.visible && s.focused;
  if (active && (s.refull || s.lastFull === null || s.now - s.lastFull >= TREE_FULL_MS)) return "full";
  if (s.urgent || (active && s.now - s.lastAsk >= TREE_POLL_MS)) return "poll";
  return "none";
}

/** `path` and everything under it. */
function dropTree(expanded: Set<string>, path: string) {
  for (const e of [...expanded]) if (e === path || e.startsWith(`${path}/`)) expanded.delete(e);
}
const parentOf = (path: string) => path.slice(0, Math.max(0, path.lastIndexOf("/")));
const saveOf = (root: string, expanded: ReadonlySet<string>): TreeState => ({ root, expanded: [...expanded].slice(0, MAX_SAVED) });

/** A reply is tagged with the session and the root it was asked under. */
export type ReplyTag = { session: string; root: string | null; expanded: ReadonlySet<string> };

/**
 * Folds a reply into the data. A reply for another session, or asked under a root that is no longer the data's, changes
 * nothing. A reply with a new root closes every folder. A `same` folder keeps its listing; a `gone` or `refused` one leaves
 * `expanded`. `save` is what to write to the layout, when something it holds changed.
 */
export function applyReply(data: TreeData, tag: ReplyTag, reply: TreeReply | "no-folder"): { data: TreeData; save: TreeState | null } {
  if (tag.session !== data.session || tag.root !== data.root) return { data, save: null };
  if (reply === "no-folder") {
    if (data.noFolder && data.listings.size === 0) return { data, save: null };
    return { data: { ...data, noFolder: true, listings: new Map() }, save: null };
  }

  let listings = new Map(data.listings);
  const expanded = new Set(data.expanded);
  let root = data.root;
  let noFolder = data.noFolder;
  let dirs: TreeDir[] = reply.dirs;
  let touched = false;
  let stateChanged = false;

  if (root === null) {
    root = reply.root;
    touched = stateChanged = true;
  } else if (root !== reply.root) {
    // A new root (a `cd` outside git): nothing under the old one means anything.
    root = reply.root;
    listings = new Map();
    expanded.clear();
    dirs = dirs.filter((d) => d.dir === "");
    touched = stateChanged = true;
  }

  for (const d of dirs) {
    if (d.dir !== "" && !expanded.has(d.dir)) continue;
    if (d.state === "same") {
      if (d.dir === "" && listings.has("") && noFolder) {
        noFolder = false;
        touched = true;
      }
      continue;
    }
    if (d.state === "listed") {
      listings.set(d.dir, { entries: d.entries, more: d.more, stamp: d.stamp, state: "listed" });
      touched = true;
      if (d.dir === "") noFolder = false;
      if (d.more === 0) {
        // A folder that left its parent's listing leaves `expanded`, with what was open under it.
        const folders = new Set(d.entries.filter((e) => e.kind === "folder").map((e) => e.name));
        for (const e of [...expanded]) {
          if (parentOf(e) === d.dir && !folders.has(e.slice(d.dir === "" ? 0 : d.dir.length + 1)) && expanded.has(e) && tag.expanded.has(e)) {
            dropTree(expanded, e);
            stateChanged = true;
          }
        }
      }
      continue;
    }
    // gone or refused
    touched = true;
    if (d.dir === "") {
      listings.set("", { entries: [], more: 0, stamp: 0, state: d.state });
      noFolder = true;
    } else {
      for (const k of [...listings.keys()]) if (k === d.dir || k.startsWith(`${d.dir}/`)) listings.delete(k);
      dropTree(expanded, d.dir);
      stateChanged = true;
    }
  }

  if (!touched) return { data, save: null };
  return { data: { session: data.session, root, noFolder, listings, expanded }, save: stateChanged ? saveOf(root, expanded) : null };
}

export type TreeEnv = { now(): number; visible(): boolean; focused(): boolean };
const browserEnv: TreeEnv = {
  now: () => Date.now(),
  visible: () => typeof document === "undefined" || document.visibilityState === "visible",
  focused: () => typeof document === "undefined" || document.hasFocus(),
};

export type EngineDeps = {
  session: string;
  saved: TreeState | undefined;
  source: TreeSource | null;
  drawn: boolean;
  onSaved(session: string, next: TreeState): void;
  onChange(data: TreeData): void;
  env?: TreeEnv;
};

export type TreeEngine = {
  data(): TreeData;
  set(p: { source?: TreeSource | null; drawn?: boolean }): void;
  /** Asks when the schedule says so. The hook calls it on a timer. */
  tick(): void;
  /** The window got the focus back. */
  focus(): void;
  expand(path: string): void;
  collapse(path: string): void;
  collapseAll(): void;
  reveal(path: string): void;
  dispose(): void;
};

/**
 * One session's tree: its data, its requests and its saves, with no React in it. It lives for one session; a reply that
 * arrives after `dispose`, or for a source that was replaced, is dropped. Its requests go one at a time, and its save is
 * debounced and flushed, for its own session, on `dispose`.
 */
export function createTreeEngine(deps: EngineDeps): TreeEngine {
  const env = deps.env ?? browserEnv;
  let data = initialData(deps.session, deps.saved);
  let source = deps.source;
  let drawn = deps.drawn;
  let disposed = false;
  let inFlight = false;
  let lastAsk: number | null = null;
  let lastFull: number | null = null;
  let urgent = false;
  let refull = false;
  let pendingSave: TreeState | null = null;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  const flushSave = () => {
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = null;
    if (pendingSave === null) return;
    const next = pendingSave;
    pendingSave = null;
    deps.onSaved(deps.session, next);
  };
  const queueSave = (next: TreeState) => {
    pendingSave = next;
    if (saveTimer !== null) clearTimeout(saveTimer);
    saveTimer = setTimeout(flushSave, SAVE_DEBOUNCE_MS);
  };
  const change = (next: TreeData) => {
    data = next;
    if (!disposed) deps.onChange(next);
  };
  /** A change the user made: shown now, saved when the root is known. */
  const edit = (expanded: Set<string>) => {
    change({ ...data, expanded });
    if (data.root !== null) queueSave(saveOf(data.root, expanded));
    urgent = true;
    pump();
  };

  const apply = (tag: ReplyTag, reply: TreeReply | "no-folder") => {
    const out = applyReply(data, tag, reply);
    if (out.data !== data) change(out.data);
    if (out.save) queueSave(out.save);
  };

  /** The requests of one round, level by level: a reply that opens a folder's listing may show more open folders to load. */
  async function work(src: TreeSource, full: boolean) {
    const rootAtStart = data.root;
    for (let pass = 0; pass < MAX_PASSES; pass++) {
      const rows = visibleRows(data.listings, data.expanded);
      let chunks = nextAsks(data.listings, rows, pass === 0 && full);
      if (pass > 0) {
        chunks = chunks.map((c) => c.filter((a) => a.dir !== "" && !data.listings.has(a.dir))).filter((c) => c.length > 0);
      }
      if (chunks.length === 0) return;
      for (const chunk of chunks) {
        if (disposed || source !== src) return;
        const tag: ReplyTag = { session: deps.session, root: data.root, expanded: new Set(data.expanded) };
        // The root moved under this round: what is left of it was asked under the old one.
        if (rootAtStart !== null && tag.root !== rootAtStart) return;
        let reply: TreeReply | "no-folder";
        try {
          reply = await src.list(chunk);
        } catch {
          return;
        }
        if (disposed || source !== src) return;
        apply(tag, reply);
      }
    }
  }

  async function round(kind: "poll" | "full") {
    const src = source;
    if (!src) return;
    inFlight = true;
    urgent = false;
    if (kind === "full") refull = false;
    try {
      await work(src, kind === "full");
    } finally {
      inFlight = false;
      lastAsk = env.now();
      if (kind === "full") lastFull = lastAsk;
    }
    pump();
  }

  function pump() {
    if (disposed) return;
    const kind = schedule({ inFlight, drawn, hasSource: source !== null, visible: env.visible(), focused: env.focused(), now: env.now(), lastAsk, lastFull, urgent, refull });
    if (kind !== "none") void round(kind);
  }

  return {
    data: () => data,
    set(p) {
      if (p.source !== undefined && p.source !== source) {
        source = p.source;
        urgent = refull = true;
      }
      if (p.drawn !== undefined && p.drawn !== drawn) {
        drawn = p.drawn;
        if (drawn) urgent = true;
      }
      pump();
    },
    tick: pump,
    focus() {
      urgent = refull = true;
      pump();
    },
    expand(path) {
      if (data.expanded.has(path)) return;
      edit(new Set([...data.expanded, path]));
    },
    collapse(path) {
      if (!data.expanded.has(path)) return;
      const next = new Set(data.expanded);
      next.delete(path);
      edit(next);
    },
    collapseAll() {
      if (data.expanded.size === 0) return;
      edit(new Set());
    },
    reveal(path) {
      const missing = ancestors(path).filter((a) => !data.expanded.has(a));
      if (missing.length === 0) return;
      edit(new Set([...data.expanded, ...missing]));
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      flushSave();
    },
  };
}

/**
 * The tree of one session, kept current with the disk: asked once when drawn, then every 2,000 ms while the window is
 * visible and focused, and again at once on focus and on an expand. `session` keys everything: change it and the old
 * session's save is flushed and its replies are dropped.
 */
export function useTree(o: { session: string; source: TreeSource | null; saved: TreeState | undefined; drawn: boolean; onSaved(session: string, next: TreeState): void }): {
  root: string | null;
  noFolder: boolean;
  rows: TreeRow[];
  expand(path: string): void;
  collapse(path: string): void;
  collapseAll(): void;
  reveal(path: string): void;
} {
  const { session, source, saved, drawn, onSaved } = o;
  const latest = useRef({ saved, source, drawn, onSaved });
  latest.current = { saved, source, drawn, onSaved };
  const [snap, setSnap] = useState<TreeData>(() => initialData(session, saved));
  const engineRef = useRef<TreeEngine | null>(null);

  useEffect(() => {
    const engine = createTreeEngine({
      session,
      saved: latest.current.saved,
      source: latest.current.source,
      drawn: latest.current.drawn,
      onSaved: (s, next) => latest.current.onSaved(s, next),
      onChange: setSnap,
    });
    engineRef.current = engine;
    setSnap((prev) => (prev.session === session ? prev : engine.data()));
    const onFocus = () => engine.focus();
    const onVisible = () => {
      if (document.visibilityState === "visible") engine.focus();
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisible);
    const timer = window.setInterval(() => engine.tick(), 250);
    engine.tick();
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisible);
      engine.dispose();
      engineRef.current = null;
    };
  }, [session]);

  useEffect(() => {
    engineRef.current?.set({ source, drawn });
  }, [source, drawn, session]);

  // Until the new session's engine has spoken, show that session's saved state, never the last one's rows.
  const data = useMemo(() => (snap.session === session ? snap : initialData(session, latest.current.saved)), [snap, session]);
  const rows = useMemo(() => visibleRows(data.listings, data.expanded), [data]);
  const expand = useCallback((p: string) => engineRef.current?.expand(p), []);
  const collapse = useCallback((p: string) => engineRef.current?.collapse(p), []);
  const collapseAll = useCallback(() => engineRef.current?.collapseAll(), []);
  const reveal = useCallback((p: string) => engineRef.current?.reveal(p), []);
  return { root: data.root, noFolder: data.noFolder, rows, expand, collapse, collapseAll, reveal };
}
