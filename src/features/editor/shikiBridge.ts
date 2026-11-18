import { StateEffect, type ChangeSet, type Extension, type Range, type Text } from "@codemirror/state";
import { Decoration, ViewPlugin, type DecorationSet, type EditorView, type ViewUpdate } from "@codemirror/view";
import { tokenizer } from "../preview/highlight";
import { COLOUR_CAP, TokenStore } from "./tokens";

/** Tells the plugin that a step has coloured more lines, so it draws them. */
const stepped = StateEffect.define<null>();

const marks = new Map<string, Decoration>();
const markOf = (cls: string): Decoration => {
  let d = marks.get(cls);
  if (!d) marks.set(cls, (d = Decoration.mark({ class: cls })));
  return d;
};

const later = (run: () => void): (() => void) => {
  if (typeof requestIdleCallback === "function") {
    const id = requestIdleCallback(run);
    return () => cancelIdleCallback(id);
  }
  const id = setTimeout(run, 16);
  return () => clearTimeout(id);
};

/**
 * Hands a transaction's changes to the store, one by one, each as the lines it replaced and the lines that replaced them.
 * Earlier changes are already in `doc` when a later one is looked at, so a change sits at its new start, and its end is where it
 * ended in the old text, moved by the changes before it.
 */
export function feedStore(store: TokenStore, start: Text, changes: ChangeSet): void {
  let doc = start;
  changes.iterChanges((fromA, toA, fromB, _toB, inserted) => {
    const to = toA + (fromB - fromA);
    const first = doc.lineAt(fromB).number;
    const removed = doc.lineAt(to).number - first + 1;
    doc = doc.replace(fromB, to, inserted);
    const last = doc.lineAt(fromB + inserted.length).number;
    const lines: string[] = [];
    for (let n = first; n <= last; n++) lines.push(doc.line(n).text);
    store.edit(first - 1, removed, lines);
  });
}

class Colours {
  decorations: DecorationSet = Decoration.none;
  private store: TokenStore | null = null;
  private cancel: (() => void) | null = null;
  private dead = false;
  /** True when the decorations were built from the colours of the text and the viewport as they are. */
  private fresh = true;
  /** The text was over the cap when last looked at, so the store does not hold it. */
  private tooBig = false;

  constructor(
    private readonly view: EditorView,
    language: string | undefined,
  ) {
    if (!language) return;
    void tokenizer(language).then((tokenize) => {
      // A view destroyed while Shiki loaded is not touched.
      if (this.dead || !tokenize) return;
      this.store = new TokenStore(tokenize);
      this.load();
    });
  }

  /** The store takes the text as it is now. Over the cap there are no colours. */
  private load(): void {
    const store = this.store;
    if (!store) return;
    const doc = this.view.state.doc;
    this.tooBig = doc.length > COLOUR_CAP;
    this.decorations = Decoration.none;
    if (this.tooBig) return;
    store.reset(Array.from(doc.iterLines()));
    this.fresh = false;
    this.want();
    this.run();
  }

  update(u: ViewUpdate): void {
    const store = this.store;
    if (!store || this.dead) return;
    if (u.docChanged) {
      const big = u.state.doc.length > COLOUR_CAP;
      if (big || this.tooBig) {
        // Over the cap: nothing is coloured. Back under it, the store starts again from the whole text.
        this.tooBig = big;
        this.decorations = Decoration.none;
        if (!big) this.load();
        return;
      }
      this.feed(u);
      // The colours on screen stay where the text moved them until the new ones are ready, so typing does not flash plain text.
      this.decorations = this.decorations.map(u.changes);
      this.fresh = false;
    } else if (u.viewportChanged) this.fresh = false;
    if (this.tooBig) return;
    this.want();
    if (!this.fresh) {
      this.draw();
      this.run();
    }
  }

  private feed(u: ViewUpdate): void {
    feedStore(this.store!, u.startState.doc, u.changes);
  }

  private want(): void {
    const { doc } = this.view.state;
    const { from, to } = this.view.viewport;
    this.store!.want(doc.lineAt(from).number - 1, doc.lineAt(to).number - 1);
  }

  /** Decorations for the lines of the viewport that have colours. When all of them do, they are fresh. */
  private draw(): void {
    const store = this.store!;
    const { doc } = this.view.state;
    const { from, to } = this.view.viewport;
    const first = doc.lineAt(from).number;
    const last = doc.lineAt(to).number;
    const ranges: Range<Decoration>[] = [];
    let complete = true;
    for (let n = first; n <= last; n++) {
      const tokens = store.line(n - 1);
      if (!tokens) {
        complete = false;
        continue;
      }
      const line = doc.line(n);
      for (const t of tokens) {
        const end = Math.min(t.to, line.length);
        if (t.from < end) ranges.push(markOf(t.cls).range(line.from + t.from, line.from + end));
      }
    }
    // Partial colours replace what is there only when nothing is there: otherwise the older ones stay until the lines are ready.
    if (complete || this.decorations.size === 0) this.decorations = Decoration.set(ranges, true);
    this.fresh = complete;
  }

  /** One step now and then, when the browser is idle: a step colours up to 100 lines. */
  private run(): void {
    if (this.cancel || this.dead || !this.store || this.tooBig) return;
    this.cancel = later(() => {
      this.cancel = null;
      const store = this.store;
      if (!store || this.dead || this.tooBig) return;
      const wasFresh = this.fresh;
      let more: boolean;
      try {
        more = store.step();
      } catch (e) {
        // Shiki's tokenizer threw: this editor goes on as plain text, and typing is not touched.
        console.warn("Syntax colours are off for this file", e);
        this.store = null;
        this.fresh = true;
        this.decorations = Decoration.none;
        this.view.dispatch({ effects: stepped.of(null) });
        return;
      }
      // Lines below the screen are coloured in the background too, so a scroll finds them done; only a step that the screen waits for redraws.
      if (!wasFresh) {
        this.draw();
        this.view.dispatch({ effects: stepped.of(null) });
      }
      if (more) this.run();
    });
  }

  destroy(): void {
    this.dead = true;
    this.cancel?.();
    this.cancel = null;
    this.store = null;
  }
}

/**
 * Syntax colours from Shiki, a hundred lines at a time. The bridge starts with none, asks for the language's tokenizer, and
 * builds its store from the text when that arrives. Each transaction's changes go to the store as the lines they replaced, and
 * only the lines on screen get decorations. Over COLOUR_CAP there are no colours.
 */
export function shiki(language: string | undefined): Extension {
  return ViewPlugin.fromClass(
    class extends Colours {
      constructor(view: EditorView) {
        super(view, language);
      }
    },
    { decorations: (c) => c.decorations },
  );
}
