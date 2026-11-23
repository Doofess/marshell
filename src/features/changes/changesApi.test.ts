import { describe, expect, it } from "vitest";
import { ApiError } from "../../lib/api/client";
import { refusalOf } from "./changesApi";

describe("refusalOf", () => {
  it("reads the reason of a 409 with a JSON body", () => {
    expect(refusalOf(new ApiError(409, '{"reason":"waiting"}'))).toBe("waiting");
    expect(refusalOf(new ApiError(409, '{"reason":"no_agent"}'))).toBe("no_agent");
    expect(refusalOf(new ApiError(409, '{"reason":"not_ready"}'))).toBe("not_ready");
  });
  it("is null for another status, another body or another error", () => {
    expect(refusalOf(new ApiError(404, '{"reason":"waiting"}'))).toBeNull();
    expect(refusalOf(new ApiError(409, "plain text"))).toBeNull();
    expect(refusalOf(new ApiError(409, '{"reason":"other"}'))).toBeNull();
    expect(refusalOf(new Error("x"))).toBeNull();
  });
});
