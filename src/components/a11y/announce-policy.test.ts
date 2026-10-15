import { describe, expect, it } from "vitest";
import { ANNOUNCE_INTERVAL_MS, createAnnouncer, shouldAnnounce, type AnnounceKind } from "./announce-policy";

const clock = (start = 10_000) => {
  let t = start;
  return { now: () => t, advance: (ms: number) => (t += ms) };
};

describe("createAnnouncer", () => {
  it("is silent until something is said", () => {
    const a = createAnnouncer({ now: clock().now });
    expect(a.current()).toEqual({ text: "", priority: "polite", seq: 0 });
  });

  it("says a message at once when nothing was said recently", () => {
    const a = createAnnouncer({ now: clock().now });
    a.say("billing is waiting for your approval");
    expect(a.current().text).toBe("billing is waiting for your approval");
    expect(a.current().seq).toBe(1);
  });

  it("drops a message identical to the one just said", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("infra has a question");
    c.advance(ANNOUNCE_INTERVAL_MS * 2);
    a.say("infra has a question");
    expect(a.current().seq).toBe(1);
  });

  it("holds a message that arrives within 1.5 s and lets the latest one win", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("one");
    c.advance(200);
    a.say("two");
    c.advance(200);
    a.say("three");
    expect(a.current().text).toBe("one");
    expect(a.pendingDelay()).toBe(ANNOUNCE_INTERVAL_MS - 400);
    c.advance(ANNOUNCE_INTERVAL_MS - 400);
    a.flush();
    expect(a.current().text).toBe("three");
    expect(a.current().seq).toBe(2);
    expect(a.pendingDelay()).toBeNull();
  });

  it("flush says nothing before the interval is over", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("one");
    a.say("two");
    c.advance(100);
    a.flush();
    expect(a.current().text).toBe("one");
  });

  it("an assertive message is not held back", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("one");
    c.advance(10);
    a.say("api-server has stopped", { priority: "assertive" });
    expect(a.current()).toMatchObject({ text: "api-server has stopped", priority: "assertive", seq: 2 });
  });

  it("an assertive message replaces a held polite one", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("one");
    c.advance(10);
    a.say("two");
    a.say("stopped", { priority: "assertive" });
    c.advance(ANNOUNCE_INTERVAL_MS);
    a.flush();
    expect(a.current().text).toBe("stopped");
  });

  it("says the same words again when the user asked for them (repeat)", () => {
    const c = clock();
    const a = createAnnouncer({ now: c.now });
    a.say("Session 3, api-server: Working. Running npm test", { repeat: true });
    c.advance(ANNOUNCE_INTERVAL_MS);
    a.say("Session 3, api-server: Working. Running npm test", { repeat: true });
    expect(a.current().seq).toBe(2);
  });

  it("ignores empty text and tells subscribers about each spoken message", () => {
    const heard: string[] = [];
    const a = createAnnouncer({ now: clock().now });
    a.subscribe((m) => heard.push(m.text));
    a.say("");
    a.say("  ");
    a.say("hello");
    expect(heard).toEqual(["hello"]);
  });
});

describe("shouldAnnounce: 'only what needs you'", () => {
  const needsYou: AnnounceKind[] = ["request", "question", "stopped", "ended", "switched", "none-selected", "on-request"];
  const quiet: AnnounceKind[] = ["progress", "working", "done", "other-pane"];

  it("announces what needs you in the default mode", () => {
    for (const k of needsYou) expect(shouldAnnounce(k, "needs-you"), k).toBe(true);
  });
  it("never announces progress, working, done or what happens in other panes in the default mode", () => {
    for (const k of quiet) expect(shouldAnnounce(k, "needs-you"), k).toBe(false);
  });
  it("on-request mode says only what was asked for", () => {
    for (const k of [...needsYou.filter((k) => k !== "on-request"), ...quiet]) expect(shouldAnnounce(k, "on-request"), k).toBe(false);
    expect(shouldAnnounce("on-request", "on-request")).toBe(true);
  });
  it("'everything' is typed but not built yet: it behaves like needs-you for now", () => {
    expect(shouldAnnounce("progress", "everything")).toBe(false);
    expect(shouldAnnounce("request", "everything")).toBe(true);
  });
});
