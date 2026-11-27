import { afterEach, describe, expect, it, vi } from "vitest";
import type { DocSave } from "../../generated/DocSave";
import type { Endpoint } from "../../generated/Endpoint";
import { libraryView, previewCopyFix, previewDelete, reviewCopies, saveDoc } from "./instructions";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const reply = (body: unknown, status = 200) => vi.fn().mockImplementation(async () => new Response(typeof body === "string" ? body : JSON.stringify(body), { status }));
const call = (m: ReturnType<typeof vi.fn>) => {
  const [url, init] = m.mock.calls[0]!;
  return { url: url as string, method: init.method as string, auth: init.headers?.Authorization as string, body: init.body ? JSON.parse(init.body) : undefined };
};
const req: DocSave = { scope: "user", name: "a", description: "", clis: [], globs: [], enabled: true, body: "" };

afterEach(() => vi.unstubAllGlobals());

describe("instructions api", () => {
  it("libraryView encodes the project and leaves it out when null", async () => {
    const f = reply({ user_docs: [] });
    vi.stubGlobal("fetch", f);
    await libraryView(endpoint, "C:\\my proj");
    expect(call(f).url).toContain("/v1/instructions?project=C%3A%5Cmy%20proj");
    expect(call(f).method).toBe("GET");
    expect(call(f).auth).toBe("Bearer t");
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await libraryView(endpoint, null);
    expect(call(g).url.endsWith("/v1/instructions")).toBe(true);
  });
  it("the preview routes post their bodies", async () => {
    const f = reply({ changeset_id: "c" });
    vi.stubGlobal("fetch", f);
    await reviewCopies(endpoint, null);
    expect(call(f)).toMatchObject({ method: "POST", body: {} });
    expect(call(f).url).toContain("/v1/instructions/review");
    const g = reply({});
    vi.stubGlobal("fetch", g);
    await reviewCopies(endpoint, "C:/p");
    expect(call(g).body).toEqual({ project: "C:/p" });
    const h = reply({});
    vi.stubGlobal("fetch", h);
    await previewDelete(endpoint, { scope: "user", name: "a" });
    expect(call(h).url).toContain("/v1/instructions/delete");
    expect(call(h).body).toEqual({ scope: "user", name: "a" });
    const i = reply({});
    vi.stubGlobal("fetch", i);
    await previewCopyFix(endpoint, { scope: "user", name: "a", cli: "claude", take_copy: true });
    expect(call(i).url).toContain("/v1/instructions/copy");
    expect(call(i).body.take_copy).toBe(true);
  });
  it("saveDoc returns the saved doc", async () => {
    vi.stubGlobal("fetch", reply({ snapshot_id: "s", view: {} }));
    expect(await saveDoc(endpoint, req)).toEqual({ ok: true, saved: { snapshot_id: "s", view: {} } });
  });
  it("saveDoc maps a 409 change to a conflict", async () => {
    vi.stubGlobal("fetch", reply({ reason: "changed", paths: ["C:/a.md"] }, 409));
    expect(await saveDoc(endpoint, req)).toEqual({ ok: false, conflict: true });
  });
  it("saveDoc maps a plain-text 409 to its message", async () => {
    vi.stubGlobal("fetch", reply("A file named api.md is already there.", 409));
    expect(await saveDoc(endpoint, req)).toEqual({ ok: false, conflict: false, message: "A file named api.md is already there." });
  });
  it("saveDoc maps a 422 and a 404 to their messages", async () => {
    vi.stubGlobal("fetch", reply("Nothing changes.", 422));
    expect(await saveDoc(endpoint, req)).toEqual({ ok: false, conflict: false, message: "Nothing changes." });
    vi.stubGlobal("fetch", reply("gone", 404));
    expect(await saveDoc(endpoint, req)).toEqual({ ok: false, conflict: false, message: "gone" });
  });
  it("saveDoc rethrows other errors", async () => {
    vi.stubGlobal("fetch", reply("boom", 500));
    await expect(saveDoc(endpoint, req)).rejects.toThrow();
  });
});
