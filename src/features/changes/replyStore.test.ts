import { describe, expect, it } from "vitest";
import { type ReplyState, afterReply, beginPost } from "./boxModel";
import { heldSessions, listen, replyHolder, setHeld } from "./replyStore";

const empty = (): ReplyState => ({ box: null, texts: new Map(), out: new Set() });

describe("replyStore", () => {
  it("a reply out is still out for a panel mounted again", () => {
    const s = "store-remount";
    const holder = replyHolder(s);
    setHeld(s, beginPost(holder.current, "T1")!);
    // The panel unmounts and a new one mounts for the same session.
    const again = replyHolder(s);
    expect(again).toBe(holder);
    expect(again.current.out.has("T1")).toBe(true);
    expect(beginPost(again.current, "T1")).toBeNull();
    setHeld(s, afterReply(again.current, "T1", { ok: true }));
    expect(heldSessions().includes(s)).toBe(false);
  });

  it("another session has its own state", () => {
    const a = "store-a";
    const b = "store-b";
    setHeld(a, beginPost(replyHolder(a).current, "T1")!);
    expect(replyHolder(b).current.out.size).toBe(0);
    expect(beginPost(replyHolder(b).current, "T1")).not.toBeNull();
    expect(replyHolder(a).current.out.has("T1")).toBe(true);
    setHeld(a, empty());
    setHeld(b, empty());
    expect(heldSessions().includes(a)).toBe(false);
    expect(heldSessions().includes(b)).toBe(false);
  });

  it("an answer that lands with no panel mounted is there for the next one", () => {
    const s = "store-late";
    const stop = listen(s, () => {});
    setHeld(s, { box: null, texts: new Map([["T1", "thanks"]]), out: new Set(["T1"]) });
    stop();
    // No panel is mounted; the reply is out, so the holder is kept.
    expect(heldSessions().includes(s)).toBe(true);
    setHeld(s, afterReply(replyHolder(s).current, "T1", { ok: false, reason: "The pull request changed." }));
    expect(heldSessions().includes(s)).toBe(true);
    expect(replyHolder(s).current.out.size).toBe(0);
    expect(replyHolder(s).current.texts.get("T1")).toBe("thanks");
    setHeld(s, afterReply(replyHolder(s).current, "T1", { ok: true }));
    expect(heldSessions().includes(s)).toBe(false);
    expect(replyHolder(s).current.texts.size).toBe(0);
    setHeld(s, empty());
  });

  it("a listener hears a change and stops hearing after it leaves", () => {
    const s = "store-listen";
    let heard = 0;
    const stop = listen(s, () => (heard += 1));
    setHeld(s, beginPost(replyHolder(s).current, "T1")!);
    expect(heard).toBe(1);
    stop();
    setHeld(s, afterReply(replyHolder(s).current, "T1", { ok: true }));
    expect(heard).toBe(1);
    expect(heldSessions().includes(s)).toBe(false);
  });

  it("a session with a listener is kept while empty, and forgotten when the listener leaves", () => {
    const s = "store-empty";
    const stop = listen(s, () => {});
    setHeld(s, empty());
    expect(heldSessions().includes(s)).toBe(true);
    stop();
    expect(heldSessions().includes(s)).toBe(false);
  });
});
