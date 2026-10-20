import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import { decide, release } from "./permissions";

const endpoint: Endpoint = { port: 9, token: "t", platform: "windows" };
const answer = (status: number, text = "") => vi.fn().mockResolvedValue(new Response(status === 204 ? null : text, { status }));
const allow: PermissionDecision = { behavior: "allow", scope: "once", message: null };

afterEach(() => vi.unstubAllGlobals());

describe("permissions api", () => {
  it("posts a decision to the request's route with the token", async () => {
    const f = answer(204);
    vi.stubGlobal("fetch", f);
    await expect(decide(endpoint, "r/1", allow)).resolves.toBe("ok");
    const [url, init] = f.mock.calls[0]!;
    expect(url).toBe("http://127.0.0.1:9/v1/permissions/r%2F1/decision");
    expect(init.method).toBe("POST");
    expect(init.headers.Authorization).toBe("Bearer t");
    expect(JSON.parse(init.body)).toEqual(allow);
  });
  it("maps 404 to gone and 409 to a conflict carrying the core's text", async () => {
    vi.stubGlobal("fetch", answer(404, "no such request"));
    await expect(decide(endpoint, "r1", allow)).resolves.toBe("gone");
    vi.stubGlobal("fetch", answer(409, "no rule to add"));
    await expect(decide(endpoint, "r1", allow)).resolves.toEqual({ conflict: "no rule to add" });
  });
  it("rejects on any other failure", async () => {
    vi.stubGlobal("fetch", answer(500, "boom"));
    await expect(decide(endpoint, "r1", allow)).rejects.toThrow();
  });
  it("releases, and maps 404 to gone", async () => {
    const f = answer(204);
    vi.stubGlobal("fetch", f);
    await expect(release(endpoint, "r1")).resolves.toBe("ok");
    expect(f.mock.calls[0]![0]).toBe("http://127.0.0.1:9/v1/permissions/r1/release");
    vi.stubGlobal("fetch", answer(404, "no such request"));
    await expect(release(endpoint, "r1")).resolves.toBe("gone");
  });
});
