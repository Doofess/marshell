import type { Endpoint } from "../../generated/Endpoint";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../lib/api/client";
import { pasteAttachment, saveAttachment } from "./regionsApi";

const ep: Endpoint = { port: 4567, token: "tok", platform: "windows" };
afterEach(() => vi.unstubAllGlobals());

describe("regionsApi", () => {
  it("posts the PNG raw with the bearer token", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ name: "1.png", path: "C:\\1.png", reference: "x" }), { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const png = new Blob(["png"], { type: "image/png" });
    const saved = await saveAttachment(ep, "t 1", png);
    expect(saved.name).toBe("1.png");
    const [url, init] = fetchMock.mock.calls[0] as [string, { method: string; headers: Record<string, string>; body: Blob }];
    expect(url).toBe("http://127.0.0.1:4567/v1/sessions/t%201/attachments");
    expect(init.method).toBe("POST");
    expect(init.headers["Content-Type"]).toBe("image/png");
    expect(init.headers.Authorization).toBe("Bearer tok");
    expect(init.body).toBe(png);
  });
  it("rejects with the body text on a non-2xx answer", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("It is paused.", { status: 409 })));
    const e = await saveAttachment(ep, "t", new Blob(["x"])).catch((x: unknown) => x);
    expect(e).toBeInstanceOf(ApiError);
    expect((e as ApiError).message).toBe("It is paused.");
  });
  it("pastes by JSON", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);
    await pasteAttachment(ep, "t 1", { attachment: "1.png", text: "hi" });
    expect((fetchMock.mock.calls[0] as [string])[0]).toBe("http://127.0.0.1:4567/v1/sessions/t%201/paste");
  });
});
