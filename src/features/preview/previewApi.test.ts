import { afterEach, describe, expect, it, vi } from "vitest";
import { resetEndpoint } from "../../lib/api/client";
import { IMAGE_CAP, PDF_CAP, TEXT_CAP, fetchContent, listFiles, resolvePaths, statFile } from "./previewApi";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const fetchMock = vi.fn();

function arrange(res: Response) {
  invoke.mockResolvedValue({ port: 4321, token: "secret", platform: "windows" });
  fetchMock.mockResolvedValue(res);
  vi.stubGlobal("fetch", fetchMock);
}
const urlOf = () => new URL(fetchMock.mock.calls[0]![0] as string);

afterEach(() => {
  vi.unstubAllGlobals();
  fetchMock.mockReset();
  invoke.mockReset();
  resetEndpoint();
});

describe("the caps", () => {
  it("mirror the core's: 2 MiB of text and 20 MiB of image", () => {
    expect(TEXT_CAP).toBe(2 * 1024 * 1024);
    expect(IMAGE_CAP).toBe(20 * 1024 * 1024);
    expect(PDF_CAP).toBe(52_428_800);
  });
});

describe("statFile", () => {
  it("asks the core with the token and the path, encoded", async () => {
    arrange(new Response(JSON.stringify({ size: 12, modified_ms: 7 }), { status: 200 }));
    expect(await statFile("C:\\dev\\a b.rs")).toEqual({ kind: "ok", size: 12, modified_ms: 7 });
    expect(urlOf().pathname).toBe("/v1/files/stat");
    expect(urlOf().searchParams.get("path")).toBe("C:\\dev\\a b.rs");
    expect(fetchMock.mock.calls[0]![1].headers.Authorization).toBe("Bearer secret");
  });
  it("says missing on a 404", async () => {
    arrange(new Response("", { status: 404 }));
    expect(await statFile("nope")).toEqual({ kind: "missing" });
  });
  it("throws on a refusal", async () => {
    arrange(new Response("outside", { status: 400 }));
    await expect(statFile("x")).rejects.toMatchObject({ status: 400 });
  });
});

describe("fetchContent", () => {
  it("returns the bytes and the modification time, and names the kind in the query", async () => {
    arrange(new Response(new Uint8Array([104, 105]), { status: 200, headers: { "X-Modified-Ms": "99" } }));
    const out = await fetchContent("a.txt", "text");
    expect(out.kind).toBe("ok");
    if (out.kind === "ok") {
      expect(new Uint8Array(out.bytes)).toEqual(new Uint8Array([104, 105]));
      expect(out.modifiedMs).toBe(99);
    }
    expect(urlOf().pathname).toBe("/v1/files/content");
    expect(urlOf().searchParams.get("kind")).toBe("text");
  });
  it("reports a 413 with the size and the limit the core named", async () => {
    arrange(new Response(JSON.stringify({ size: 5000000, limit: 2097152 }), { status: 413 }));
    expect(await fetchContent("big.txt", "text")).toEqual({ kind: "tooLarge", size: 5000000, limit: 2097152 });
  });
  it("says missing on a 404", async () => {
    arrange(new Response("", { status: 404 }));
    expect(await fetchContent("gone", "image")).toEqual({ kind: "missing" });
  });
});

describe("resolvePaths", () => {
  it("posts the candidates to the session and returns its paths", async () => {
    arrange(new Response(JSON.stringify({ paths: ["C:\\p\\a.rs", null] }), { status: 200 }));
    expect(await resolvePaths("tab 1", ["a.rs", "zzz"])).toEqual(["C:\\p\\a.rs", null]);
    expect(urlOf().pathname).toBe("/v1/sessions/tab%201/resolve");
    expect(fetchMock.mock.calls[0]![1].method).toBe("POST");
    expect(JSON.parse(fetchMock.mock.calls[0]![1].body)).toEqual({ candidates: ["a.rs", "zzz"] });
  });
});

describe("listFiles", () => {
  it("gets the session's file list", async () => {
    const list = { root: "C:\\p", files: ["a.rs"], truncated: false };
    arrange(new Response(JSON.stringify(list), { status: 200 }));
    expect(await listFiles("t1")).toEqual(list);
    expect(urlOf().pathname).toBe("/v1/sessions/t1/files");
  });
});
