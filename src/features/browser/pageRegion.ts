import { initialRegion, type Region } from "../regions/region";

/**
 * A region being marked on a web page. `wait`: the page is being frozen as a picture; `box`: the box is up over the picture; `comment`:
 * its comment is being written. `url` is the page's address when the selection began: the picture belongs to it. Null: no selection.
 */
export type PageRegion = { step: "wait" | "box" | "comment"; region: Region; busy: boolean; error: string | null; url: string } | null;

export type PageRegionEvent =
  | { type: "start"; url: string }
  | { type: "picture"; ok: boolean }
  | { type: "confirm" }
  | { type: "back" }
  | { type: "cancel" }
  | { type: "moved"; url: string }
  | { type: "hidden" }
  | { type: "busy" }
  | { type: "failed"; error: string }
  | { type: "added" };

/**
 * What a selection does on each event. A start waits for the picture; the picture makes the box, and no picture ends it (the caller
 * says why). The page going to another address, off screen, or the selection being cancelled or delivered, ends it. A failed add
 * keeps the comment and the box where they were, with the reason.
 */
export function pageRegionStep(s: PageRegion, e: PageRegionEvent): PageRegion {
  if (e.type === "start") return { step: "wait", region: initialRegion(1, { top: 0, bottom: 1 }), busy: false, error: null, url: e.url };
  if (s === null) return null;
  switch (e.type) {
    case "picture":
      if (s.step !== "wait") return s;
      return e.ok ? { ...s, step: "box" } : null;
    case "confirm":
      return s.step === "box" ? { ...s, step: "comment", error: null } : s;
    case "back":
      return s.step === "comment" ? { ...s, step: "box", error: null } : s;
    case "moved":
      return e.url === s.url ? s : null;
    case "busy":
      return { ...s, busy: true, error: null };
    case "failed":
      return { ...s, busy: false, error: e.error };
    case "cancel":
    case "hidden":
    case "added":
      return null;
  }
}
