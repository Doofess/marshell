import { ChangeSet, EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { feedStore } from "./shikiBridge";
import { TokenStore } from "./tokens";

/** A tokenizer whose colours are the lines themselves, so a store's answer shows which text it holds. */
const echo = (lines: string[]) => ({ tokens: lines.map((l) => [{ from: 0, to: l.length, cls: l }]), state: undefined });

describe("shikiBridge", () => {
  it("feeds_each_change_as_the_lines_it_replaced", () => {
    // mulberry32: the low bits of a plain congruential generator repeat, and the edits would too.
    let seed = 7;
    const random = (n: number) => {
      seed = (seed + 0x6d2b79f5) | 0;
      let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return Math.floor((((t ^ (t >>> 14)) >>> 0) / 4294967296) * n);
    };
    const alphabet = ["a", "b", "\n", "\n", "cd"];
    for (let round = 0; round < 200; round++) {
      let text = Array.from({ length: 1 + random(30) }, (_, i) => `line ${i}`).join("\n");
      const state = EditorState.create({ doc: text });
      const store = new TokenStore(echo);
      store.reset(Array.from(state.doc.iterLines()));
      const specs = [];
      let at = 0;
      for (let k = 0; k < 1 + random(4); k++) {
        const from = at + random(Math.max(1, text.length - at));
        const to = Math.min(text.length, from + random(6));
        const insert = Array.from({ length: random(4) }, () => alphabet[random(alphabet.length)]).join("");
        specs.push({ from, to, insert });
        at = to;
        if (at >= text.length) break;
      }
      const changes = ChangeSet.of(specs, text.length);
      const next = state.update({ changes }).state.doc;
      feedStore(store, state.doc, changes);
      while (store.step());
      const want = Array.from(next.iterLines());
      for (let n = 0; n < want.length; n++) expect(store.line(n)?.[0]?.cls ?? "", `round ${round} line ${n}`).toBe(want[n] === "" ? "" : want[n]);
      expect(store.line(want.length)).toBeNull();
      text = next.toString();
    }
  });
});
