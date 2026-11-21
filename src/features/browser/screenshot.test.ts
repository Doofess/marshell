import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api/client";
import { SHOT_COVERED, SHOT_FAILED, savedText, saveShot, shotAsk, shotReason, type ShotDeps } from "./screenshot";

const PNG_B64 = "iVBORw0KGgo=";
const NAME = "example.com 2026-11-21 134501.png";
const target = { key: "s/p", url: "https://example.com/" };

function fake(over: Partial<ShotDeps> = {}): ShotDeps {
  return {
    uncovered: async () => true,
    paint: async () => {},
    shoot: async () => ({ png: PNG_B64, cut: false }),
    save: async () => ({ name: NAME }),
    now: () => new Date(2026, 10, 21, 13, 45, 1),
    ...over,
  };
}

describe("screenshot", () => {
  it("sends the page's host and the local time for the name", () => {
    const at = new Date(2026, 10, 21, 13, 45, 1);
    expect(shotAsk("https://example.com/a/b?c", at)).toEqual({ host: "example.com", at: "2026-11-21 134501" });
    expect(shotAsk("http://localhost:5173/", at)).toEqual({ host: "localhost:5173", at: "2026-11-21 134501" });
    expect(shotAsk("about:blank", at).host).toBe("");
    expect(shotAsk("not an address", at).host).toBe("");
    expect(shotAsk("https://example.com/", new Date(2026, 0, 2, 3, 4, 5)).at).toBe("2026-01-02 030405");
  });

  it("says why a picture can't be taken", () => {
    const page = { kind: "page", key: "s/p", url: "https://example.com/", loaded: true } as const;
    expect(shotReason(page, true, true)).toBeNull();
    expect(shotReason(page, false, true)).toBe("Not available on this system yet");
    expect(shotReason({ kind: "file" }, true, true)).toBe("Not for an HTML file");
    expect(shotReason({ kind: "none" }, true, true)).toBe("No page is open");
    expect(shotReason({ ...page, loaded: false }, true, true)).toBe("The page isn't loaded");
    expect(shotReason(page, true, false)).toBe("Show the Browser panel to save a screenshot.");
  });

  it("words what was saved", () => {
    expect(savedText(NAME, false)).toBe("Saved example.com 2026-11-21 134501.png to Downloads.");
    expect(savedText(NAME, true)).toBe("Saved the first 16,384 pixels of the page as example.com 2026-11-21 134501.png to Downloads.");
  });

  it("waits_for_the_cover_to_lift, then two paints, before it asks the shell", async () => {
    const order: string[] = [];
    let lift!: () => void;
    const deps = fake({
      uncovered: () => new Promise<boolean>((r) => (lift = () => (order.push("uncovered"), r(true)))),
      paint: async () => void order.push("paint"),
      shoot: async () => (order.push("shoot"), { png: PNG_B64, cut: false }),
    });
    const said = saveShot(deps, target, false);
    await Promise.resolve();
    expect(order).toEqual([]);
    lift();
    expect(await said).toBe("Saved example.com 2026-11-21 134501.png to Downloads.");
    expect(order).toEqual(["uncovered", "paint", "paint", "shoot"]);
  });

  it("gives up when the page stays covered, and takes no picture", async () => {
    const shoot = vi.fn(async () => ({ png: PNG_B64, cut: false }));
    expect(await saveShot(fake({ uncovered: async () => false, shoot }), target, false)).toBe(SHOT_COVERED);
    expect(shoot).not.toHaveBeenCalled();
  });

  it("sends the host and time, a PNG blob, and uses the name the core wrote", async () => {
    const save = vi.fn(async (_h: string, _a: string, _b: Blob) => ({ name: "example.com 2026-11-21 134501 (2).png" }));
    const said = await saveShot(fake({ save }), target, false);
    expect(said).toBe("Saved example.com 2026-11-21 134501 (2).png to Downloads.");
    const [host, at, blob] = save.mock.calls[0]!;
    expect([host, at, blob.type, blob.size]).toEqual(["example.com", "2026-11-21 134501", "image/png", 8]);
  });

  it("says a cut picture was cut", async () => {
    const said = await saveShot(fake({ shoot: async () => ({ png: PNG_B64, cut: true }) }), target, true);
    expect(said).toBe(savedText(NAME, true));
  });

  it("says a page that went off screen, the core's words for a refusal, and a plain line for anything else", async () => {
    expect(await saveShot(fake({ shoot: async () => Promise.reject("hidden") }), target, false)).toBe("The page isn't on screen. Show it and try again.");
    expect(await saveShot(fake({ save: async () => Promise.reject(new ApiError(413, "That picture is over 64 MB.")) }), target, false)).toBe(
      "The screenshot wasn't saved. That picture is over 64 MB.",
    );
    expect(await saveShot(fake({ shoot: async () => Promise.reject("the page's picture was not ready in 10 s") }), target, false)).toBe(SHOT_FAILED);
  });
});
