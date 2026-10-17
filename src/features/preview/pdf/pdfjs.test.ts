import { describe, expect, it } from "vitest";
import { pdfErrorMessage } from "./pdfjs";

describe("pdfErrorMessage", () => {
  it("says a locked PDF is password-protected and any other failure can't be shown", () => {
    expect(pdfErrorMessage({ name: "PasswordException" })).toBe("This PDF is password-protected.");
    expect(pdfErrorMessage({ name: "InvalidPDFException" })).toBe("This PDF can't be shown.");
    expect(pdfErrorMessage(new Error("x"))).toBe("This PDF can't be shown.");
  });
});
