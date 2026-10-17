import { describe, expect, it } from "vitest";
import { afterDeleted } from "./usePreviewFile";

describe("afterDeleted", () => {
  it("keeps the last pages of a deleted PDF", () => {
    const bytes = new ArrayBuffer(4);
    const pdf = { phase: "pdf", bytes, version: 2 } as const;
    expect(afterDeleted(pdf)).toEqual({ phase: "deleted", last: pdf });
  });
});
