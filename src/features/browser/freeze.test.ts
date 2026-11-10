import { describe, expect, it } from "vitest";
import { createFreezer, freezeStep, type Freeze, type FreezeEffect, type FreezeEvent } from "./freeze";

const live: Freeze = { phase: "live", seq: 0, picture: null };

/** Runs events in order; returns the end state and every effect asked, in order. */
function run(events: FreezeEvent[], from: Freeze = live): { s: Freeze; effects: FreezeEffect[] } {
  let s = from;
  const effects: FreezeEffect[] = [];
  for (const e of events) {
    const r = freezeStep(s, e);
    s = r.next;
    effects.push(...r.effects);
  }
  return { s, effects };
}

describe("freezeStep", () => {
  it("freezeStep_a_cover_captures_then_hides_after_the_paint", () => {
    const c = freezeStep(live, { type: "cover" });
    expect(c.effects).toEqual(["capture"]);
    expect(c.next.phase).toBe("capturing");
    const got = freezeStep(c.next, { type: "captured", seq: c.next.seq, picture: "data:image/png;base64,AA" });
    expect(got.effects).toEqual([]);
    expect(got.next).toMatchObject({ phase: "capturing", picture: "data:image/png;base64,AA" });
    const painted = freezeStep(got.next, { type: "painted", seq: got.next.seq });
    expect(painted.effects).toEqual(["hide"]);
    expect(painted.next).toMatchObject({ phase: "frozen", picture: "data:image/png;base64,AA" });
  });

  it("freezeStep_an_uncover_during_the_capture_goes_back_to_live_and_the_late_capture_does_nothing", () => {
    const c = freezeStep(live, { type: "cover" });
    const u = freezeStep(c.next, { type: "uncover" });
    expect(u.next.phase).toBe("live");
    expect(u.next.picture).toBeNull();
    expect(u.effects).toEqual([]);
    const late = freezeStep(u.next, { type: "captured", seq: c.next.seq, picture: "data:image/png;base64,AA" });
    expect(late.next).toEqual(u.next);
    expect(late.effects).toEqual([]);
    const latePaint = freezeStep(u.next, { type: "painted", seq: c.next.seq });
    expect(latePaint.next).toEqual(u.next);
    expect(latePaint.effects).toEqual([]);
  });

  it("freezeStep_a_second_cover_while_frozen_changes_nothing", () => {
    const { s } = run([{ type: "cover" }, { type: "captured", seq: 1, picture: "p" }, { type: "painted", seq: 1 }]);
    expect(s.phase).toBe("frozen");
    const again = freezeStep(s, { type: "cover" });
    expect(again.next).toEqual(s);
    expect(again.effects).toEqual([]);
    const during = freezeStep({ ...s, phase: "capturing" }, { type: "cover" });
    expect(during.effects).toEqual([]);
  });

  it("freezeStep_cover_uncover_cover_ends_frozen_with_the_newest_picture", () => {
    const first = freezeStep(live, { type: "cover" });
    const un = freezeStep(first.next, { type: "uncover" });
    const second = freezeStep(un.next, { type: "cover" });
    expect(second.effects).toEqual(["capture"]);
    expect(second.next.seq).not.toBe(first.next.seq);
    const { s, effects } = run(
      [
        { type: "captured", seq: first.next.seq, picture: "old" },
        { type: "captured", seq: second.next.seq, picture: "new" },
        { type: "painted", seq: first.next.seq },
        { type: "painted", seq: second.next.seq },
      ],
      second.next,
    );
    expect(s).toMatchObject({ phase: "frozen", picture: "new" });
    expect(effects).toEqual(["hide"]);
  });

  it("freezeStep_a_failed_capture_still_hides", () => {
    const { s, effects } = run([{ type: "cover" }, { type: "captured", seq: 1, picture: null }, { type: "painted", seq: 1 }]);
    expect(s).toMatchObject({ phase: "frozen", picture: null });
    expect(effects).toEqual(["capture", "hide"]);
  });

  it("freezeStep_thaw_shows_before_dropping_the_picture", () => {
    const frozen = run([{ type: "cover" }, { type: "captured", seq: 1, picture: "p" }, { type: "painted", seq: 1 }]).s;
    const u = freezeStep(frozen, { type: "uncover" });
    expect(u.effects).toEqual(["show"]);
    expect(u.next).toMatchObject({ phase: "thawing", picture: "p" });
    const shown = freezeStep(u.next, { type: "shown", seq: u.next.seq });
    expect(shown.effects).toEqual(["dropPicture"]);
    expect(shown.next).toMatchObject({ phase: "live", picture: null });
  });

  it("freezeStep_a_stale_shown_keeps_the_picture", () => {
    const frozen = run([{ type: "cover" }, { type: "captured", seq: 1, picture: "p" }, { type: "painted", seq: 1 }]).s;
    const u = freezeStep(frozen, { type: "uncover" });
    const c = freezeStep(u.next, { type: "cover" });
    expect(c.effects).toEqual(["capture"]);
    const late = freezeStep(c.next, { type: "shown", seq: u.next.seq });
    expect(late.next).toEqual(c.next);
    expect(late.effects).toEqual([]);
  });
});

/** A freezer whose calls are logged; `capture` answers with what the test gives. */
function rig(capture: () => Promise<string>) {
  const log: string[] = [];
  const freezer = createFreezer({
    capture: () => {
      log.push("capture");
      return capture();
    },
    show: () => {
      log.push("show");
      return Promise.resolve();
    },
    hide: () => log.push("hide"),
    paint: () => {
      log.push("paint");
      return Promise.resolve();
    },
    render: (f, now) => log.push(`render:${f.phase}:${f.picture === null ? "none" : "picture"}${now ? ":now" : ""}`),
  });
  return { log, freezer };
}
const settle = () => new Promise((r) => setTimeout(r, 0));

describe("createFreezer", () => {
  it("captures, draws the picture, waits for its paint and only then hides; a thaw shows, waits, then drops the picture", async () => {
    const { log, freezer } = rig(() => Promise.resolve("data:image/png;base64,AA"));
    freezer.send({ type: "cover" });
    await settle();
    expect(log).toEqual(["render:capturing:none", "capture", "render:capturing:picture:now", "paint", "render:frozen:picture", "hide"]);
    log.length = 0;
    freezer.send({ type: "uncover" });
    await settle();
    expect(log).toEqual(["render:thawing:picture", "show", "paint", "render:live:none"]);
  });

  it("a rejected or non-png capture still hides, over a plain fill", async () => {
    for (const capture of [() => Promise.reject(new Error("hidden")), () => Promise.resolve("")]) {
      const { log, freezer } = rig(capture);
      freezer.send({ type: "cover" });
      await settle();
      expect(log).toEqual(["render:capturing:none", "capture", "render:capturing:none:now", "paint", "render:frozen:none", "hide"]);
    }
  });

  it("an uncover while the capture is out never hides the page", async () => {
    let answer: (p: string) => void = () => {};
    const { log, freezer } = rig(() => new Promise<string>((r) => (answer = r)));
    freezer.send({ type: "cover" });
    freezer.send({ type: "uncover" });
    answer("data:image/png;base64,AA");
    await settle();
    expect(log).not.toContain("hide");
    expect(log).toEqual(["render:capturing:none", "capture", "render:live:none", "paint"]);
  });
});
