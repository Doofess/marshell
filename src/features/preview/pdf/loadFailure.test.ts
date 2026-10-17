import { describe, expect, it } from "vitest";
import { afterLoadFailure } from "./loadFailure";

describe("afterLoadFailure", () => {
  it("keeps the document on screen when a reload fails (a file caught half-written), and says nothing", () => {
    expect(afterLoadFailure(true, new Error("bad xref"))).toEqual({ keepDocument: true, error: null });
  });
  it("shows the error when nothing was loaded yet", () => {
    expect(afterLoadFailure(false, { name: "InvalidPDFException" })).toEqual({ keepDocument: false, error: "This PDF can't be shown." });
    expect(afterLoadFailure(false, { name: "PasswordException" })).toEqual({ keepDocument: false, error: "This PDF is password-protected." });
  });
});
