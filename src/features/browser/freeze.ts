export type Freeze = { phase: "live" | "capturing" | "frozen" | "thawing"; seq: number; picture: string | null };
export type FreezeEvent =
  | { type: "cover" }
  | { type: "uncover" }
  | { type: "captured"; seq: number; picture: string | null }
  | { type: "painted"; seq: number }
  | { type: "shown"; seq: number };
export type FreezeEffect = "capture" | "hide" | "show" | "dropPicture";

const none = (next: Freeze) => ({ next, effects: [] as FreezeEffect[] });

/**
 * live → (cover) capturing → (captured) picture on screen → (painted) hide → frozen → (uncover) show → (shown) dropPicture → live.
 * An event with an old `seq` does nothing. Every cover and uncover that changes the phase takes a new `seq`, so the answers to
 * an earlier round (a late capture, a late paint, a late show) are ignored. A failed capture (`picture: null`) still hides the
 * page: the slot then shows a plain fill. The order is S12's: capture first, hide only after the picture has painted; on a thaw,
 * show first, and drop the picture after the next paint.
 */
export function freezeStep(s: Freeze, e: FreezeEvent): { next: Freeze; effects: FreezeEffect[] } {
  switch (e.type) {
    case "cover":
      // A cover while a picture is up or being made changes nothing. During a thaw the page is shown again, so it is captured anew.
      if (s.phase === "live") return { next: { phase: "capturing", seq: s.seq + 1, picture: null }, effects: ["capture"] };
      if (s.phase === "thawing") return { next: { phase: "capturing", seq: s.seq + 1, picture: s.picture }, effects: ["capture"] };
      return none(s);
    case "uncover":
      if (s.phase === "frozen") return { next: { phase: "thawing", seq: s.seq + 1, picture: s.picture }, effects: ["show"] };
      // The page was never hidden: back to live, and whatever was drawn goes.
      if (s.phase === "capturing") return none({ phase: "live", seq: s.seq + 1, picture: null });
      return none(s);
    case "captured":
      if (e.seq !== s.seq || s.phase !== "capturing") return none(s);
      return none({ ...s, picture: e.picture });
    case "painted":
      if (e.seq !== s.seq || s.phase !== "capturing") return none(s);
      return { next: { ...s, phase: "frozen" }, effects: ["hide"] };
    case "shown":
      if (e.seq !== s.seq || s.phase !== "thawing") return none(s);
      return { next: { phase: "live", seq: s.seq, picture: null }, effects: ["dropPicture"] };
  }
}

/** What the freezer needs from the world, so the order of its calls can be tested without a shell or a DOM. */
export type FreezeIo = {
  /** The shown page as a picture; rejects when the page isn't the one shown. */
  capture(): Promise<string>;
  /** Shows the page. */
  show(): Promise<void>;
  /** Hides the page. */
  hide(): void;
  /** Resolves after the next frame has painted. */
  paint(): Promise<void>;
  /** Draws a new state. `now` asks for it to be on screen before this returns (the picture must be in the DOM before its paint is awaited). */
  render(f: Freeze, now: boolean): void;
};

export const LIVE: Freeze = { phase: "live", seq: 0, picture: null };

const PNG = "data:image/png;base64,";

/** Runs `freezeStep` and does what it asks, feeding the answers back as events. Any failed capture counts as no picture. */
export function createFreezer(io: FreezeIo) {
  let s = LIVE;
  const run = (effect: FreezeEffect, seq: number) => {
    if (effect === "capture") {
      io.capture()
        .then(
          (p) => (p.startsWith(PNG) ? p : null),
          () => null,
        )
        .then((picture) => {
          send({ type: "captured", seq, picture });
          return io.paint();
        })
        .then(() => send({ type: "painted", seq }));
    } else if (effect === "show") {
      io.show()
        .catch(() => {})
        .then(() => io.paint())
        .then(() => send({ type: "shown", seq }));
    } else if (effect === "hide") io.hide();
    // "dropPicture": the state that asked for it has no picture, so there is nothing left to do.
  };
  function send(e: FreezeEvent) {
    const before = s;
    const { next, effects } = freezeStep(s, e);
    s = next;
    if (next !== before) io.render(next, e.type === "captured");
    for (const f of effects) run(f, next.seq);
  }
  return {
    send,
    /** Back to live without touching the page; answers still on their way do nothing. */
    reset() {
      if (s.phase === "live") return;
      s = { phase: "live", seq: s.seq + 1, picture: null };
      io.render(s, false);
    },
  };
}
