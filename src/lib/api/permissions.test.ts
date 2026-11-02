import { afterEach, describe, expect, it, vi } from "vitest";
import type { Endpoint } from "../../generated/Endpoint";
import type { PermissionDecision } from "../../generated/PermissionDecision";
import { decide, decideAs, release } from "./permissions";

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

describe("answering for an administrator row", () => {
  it("an_elevated_approval_answer_goes_through_invoke: no request reaches the token API", async () => {
    const f = answer(204);
    vi.stubGlobal("fetch", f);
    const ask = vi.fn().mockResolvedValue(undefined);
    await expect(decideAs(endpoint, "r1", allow, true, ask)).resolves.toBe("ok");
    expect(ask).toHaveBeenCalledWith("elevated_decide", { rid: "r1", decision: allow });
    expect(f).not.toHaveBeenCalled();
  });
  it("a_plain_row_still_uses_the_api: any other row still posts to it", async () => {
    vi.stubGlobal("fetch", answer(204));
    const ask = vi.fn();
    await expect(decideAs(endpoint, "r1", allow, false, ask)).resolves.toBe("ok");
    expect(ask).not.toHaveBeenCalled();
  });
  it("an_invoke_error_shows_the_core_copy: a refusal comes back as the conflict text, a missing request as gone", async () => {
    const copy = "Not an administrator session.";
    await expect(decideAs(endpoint, "r1", allow, true, vi.fn().mockRejectedValue(copy))).resolves.toEqual({ conflict: copy });
    await expect(decideAs(endpoint, "r1", allow, true, vi.fn().mockRejectedValue("no such request"))).resolves.toBe("gone");
  });
});
