import { afterEach, describe, expect, it, vi } from "vitest";
import type { Draft } from "../../generated/Draft";
import { resetEndpoint } from "../../lib/api/client";
import { dropDraft, getDraft, listDrafts, mergeTexts, putDraft, saveFile, unsaveFile } from "./editorApi";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const fetchMock = vi.fn();

function arrange(res: Response) {
  invoke.mockResolvedValue({ port: 4321, token: "secret", platform: "windows" });
  fetchMock.mockResolvedValue(res);
  vi.stubGlobal("fetch", fetchMock);
}
const urlOf = () => new URL(fetchMock.mock.calls[0]![0] as string);
const initOf = () => fetchMock.mock.calls[0]![1] as { method: string; headers: Record<string, string>; body: unknown };

afterEach(() => {
  vi.unstubAllGlobals();
  fetchMock.mockReset();
  invoke.mockReset();
  resetEndpoint();
});

const saved = { size: 3, modified_ms: 9, hash: "a".repeat(64), snapshot: "s1" };
const draft: Draft = { path: "C:\\a.txt", text: "b", base_text: "a", base_hash: null, encoding: "utf-8", eol: "lf", edited_ms: 5 };

describe("saveFile", () => {
  it("sends the bytes as the body, with the path and the expected hash, and the token", async () => {
    arrange(new Response(JSON.stringify(saved), { status: 200 }));
    const bytes = new Uint8Array([104, 105]);
    expect(await saveFile("C:\\dev\\a b.txt", bytes, "f".repeat(64))).toEqual({ kind: "saved", saved });
    expect(urlOf().pathname).toBe("/v1/files/content");
    expect(urlOf().searchParams.get("path")).toBe("C:\\dev\\a b.txt");
    expect(urlOf().searchParams.get("expect")).toBe("f".repeat(64));
    expect(initOf().method).toBe("PUT");
    expect(initOf().headers.Authorization).toBe("Bearer secret");
    expect(initOf().body).toBe(bytes);
  });
  it("sends expect=none for a null hash", async () => {
    arrange(new Response(JSON.stringify({ ...saved, snapshot: null }), { status: 200 }));
    expect(await saveFile("a.txt", new Uint8Array(), null)).toEqual({ kind: "saved", saved: { ...saved, snapshot: null } });
    expect(urlOf().searchParams.get("expect")).toBe("none");
  });
  it("says conflict with the disk's hash on a 409", async () => {
    arrange(new Response(JSON.stringify({ hash: "c".repeat(64) }), { status: 409 }));
    expect(await saveFile("a.txt", new Uint8Array(), "f".repeat(64))).toEqual({ kind: "conflict", hash: "c".repeat(64) });
  });
  it("says conflict with no hash when the file is gone", async () => {
    arrange(new Response(JSON.stringify({ hash: null }), { status: 409 }));
    expect(await saveFile("a.txt", new Uint8Array(), "f".repeat(64))).toEqual({ kind: "conflict", hash: null });
  });
  it("says tooLarge on a 413", async () => {
    arrange(new Response("", { status: 413 }));
    expect(await saveFile("a.txt", new Uint8Array(), null)).toEqual({ kind: "tooLarge" });
  });
  it("carries the core's line on a 400, a 403 and a 500", async () => {
    for (const status of [400, 403, 500]) {
      arrange(new Response("That path can't be saved.", { status }));
      expect(await saveFile("a.txt", new Uint8Array(), null)).toEqual({ kind: "refused", line: "That path can't be saved." });
    }
  });
  it("gives a generic line for an empty body and for a network failure", async () => {
    arrange(new Response("", { status: 500 }));
    const empty = await saveFile("a.txt", new Uint8Array(), null);
    expect(empty).toEqual({ kind: "refused", line: expect.stringMatching(/^[A-Z][^!]*\.$/) });
    fetchMock.mockReset();
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    expect(await saveFile("a.txt", new Uint8Array(), null)).toEqual(empty);
  });
});

describe("unsaveFile", () => {
  it("posts the path and the snapshot and returns the file as it was left", async () => {
    arrange(new Response(JSON.stringify(saved), { status: 200 }));
    expect(await unsaveFile("a.txt", "s1")).toEqual({ kind: "saved", saved });
    expect(urlOf().pathname).toBe("/v1/files/unsave");
    expect(initOf().method).toBe("POST");
    expect(JSON.parse(initOf().body as string)).toEqual({ path: "a.txt", snapshot: "s1" });
  });
  it("says conflict on a 409 and on a 404", async () => {
    for (const status of [409, 404]) {
      arrange(new Response(JSON.stringify({ hash: null }), { status }));
      expect(await unsaveFile("a.txt", "s1")).toEqual({ kind: "conflict" });
    }
  });
  it("carries the core's line on a 400, a 403 and a 500", async () => {
    for (const status of [400, 403, 500]) {
      arrange(new Response("The file is read-only or in use by another program.", { status }));
      expect(await unsaveFile("a.txt", "s1")).toEqual({ kind: "refused", line: "The file is read-only or in use by another program." });
    }
  });
  it("gives a generic line for a network failure", async () => {
    arrange(new Response("", { status: 200 }));
    fetchMock.mockReset();
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    expect(await unsaveFile("a.txt", "s1")).toEqual({ kind: "refused", line: expect.any(String) });
  });
});

describe("mergeTexts", () => {
  it("posts the three texts and returns the merged text with its conflict count", async () => {
    arrange(new Response(JSON.stringify({ text: "m", conflicts: 2 }), { status: 200 }));
    expect(await mergeTexts("b", "a", "c")).toEqual({ kind: "merged", text: "m", conflicts: 2 });
    expect(urlOf().pathname).toBe("/v1/files/merge");
    expect(initOf().method).toBe("POST");
    expect(JSON.parse(initOf().body as string)).toEqual({ base: "b", mine: "a", disk: "c" });
  });
  it("carries the core's line on a 422", async () => {
    arrange(new Response("The texts differ too much to merge.", { status: 422 }));
    expect(await mergeTexts("b", "a", "c")).toEqual({ kind: "refused", line: "The texts differ too much to merge." });
  });
  it("says a text is over 3 MB on a 413", async () => {
    arrange(new Response("", { status: 413 }));
    expect(await mergeTexts("b", "a", "c")).toEqual({ kind: "refused", line: "A text is over 3 MB." });
  });
  it("gives a generic line for a network failure", async () => {
    arrange(new Response("", { status: 200 }));
    fetchMock.mockReset();
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    expect(await mergeTexts("b", "a", "c")).toEqual({ kind: "refused", line: expect.any(String) });
  });
});

describe("the drafts", () => {
  it("listDrafts returns the rows", async () => {
    arrange(new Response(JSON.stringify({ drafts: [{ path: "a", edited_ms: 1 }] }), { status: 200 }));
    expect(await listDrafts()).toEqual([{ path: "a", edited_ms: 1 }]);
    expect(urlOf().pathname).toBe("/v1/drafts");
  });
  it("listDrafts is empty when the request fails", async () => {
    arrange(new Response("", { status: 500 }));
    expect(await listDrafts()).toEqual([]);
  });
  it("getDraft is null on a 204, the core's answer when there is no draft, without reading a body", async () => {
    const res = new Response(null, { status: 204 });
    const json = vi.spyOn(res, "json");
    arrange(res);
    expect(await getDraft("C:\a.txt")).toBeNull();
    expect(json).not.toHaveBeenCalled();
  });
  it("getDraft returns the draft, and null on a 404", async () => {
    arrange(new Response(JSON.stringify(draft), { status: 200 }));
    expect(await getDraft("C:\\a.txt")).toEqual(draft);
    expect(urlOf().pathname).toBe("/v1/drafts/one");
    expect(urlOf().searchParams.get("path")).toBe("C:\\a.txt");
    arrange(new Response("", { status: 404 }));
    expect(await getDraft("C:\\a.txt")).toBeNull();
  });
  it("putDraft is true on a 204 and false on anything else", async () => {
    arrange(new Response(null, { status: 204 }));
    expect(await putDraft(draft)).toBe(true);
    expect(initOf().method).toBe("PUT");
    expect(JSON.parse(initOf().body as string)).toEqual(draft);
    for (const status of [400, 413, 503]) {
      arrange(new Response("full", { status }));
      expect(await putDraft(draft)).toBe(false);
    }
    fetchMock.mockReset();
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    expect(await putDraft(draft)).toBe(false);
  });
  it("dropDraft deletes by path and never throws", async () => {
    arrange(new Response(null, { status: 204 }));
    await dropDraft("C:\\a.txt");
    expect(initOf().method).toBe("DELETE");
    expect(urlOf().searchParams.get("path")).toBe("C:\\a.txt");
    fetchMock.mockReset();
    fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));
    await expect(dropDraft("C:\\a.txt")).resolves.toBeUndefined();
  });
});
