import { describe, expect, it } from "vitest";
import { headline, receiptText } from "./headline";

describe("headline", () => {
  it("names each tool as a verb phrase", () => {
    expect(headline({ kind: "bash", command: "npm test" })).toBe("Run a command");
    expect(headline({ kind: "edit", path: "src/auth.ts", add: 12, del: 3, hunk: [] })).toBe("Edit src/auth.ts (+12 \u22123)");
    expect(headline({ kind: "write", path: ".env.example", preview: "", isNew: true })).toBe("Create .env.example");
    expect(headline({ kind: "write", path: "README.md", preview: "", isNew: false })).toBe("Overwrite README.md");
    expect(headline({ kind: "fetch", url: "https://api.github.com/repos/x/y" })).toBe("Fetch api.github.com");
    expect(headline({ kind: "mcp", server: "github", tool: "create_pr", args: {} })).toBe("Use github \u00b7 create_pr");
    expect(headline({ kind: "question", question: "Which branch?", options: [] })).toBe("Has a question");
  });
  it("never throws on a URL it cannot parse (review focus 1)", () => {
    expect(headline({ kind: "fetch", url: "not a url \u{1F680}" })).toBe("Fetch not a url \u{1F680}");
  });
  it("keeps paths with spaces and non-Latin names whole", () => {
    expect(headline({ kind: "write", path: "C:/Users/Home Office/\u8a8d\u8a3c.ts", preview: "", isNew: true })).toBe("Create C:/Users/Home Office/\u8a8d\u8a3c.ts");
  });
  it("shows zero changes as +0 \u22120 rather than hiding them", () => {
    expect(headline({ kind: "edit", path: "a.ts", add: 0, del: 0, hunk: [] })).toBe("Edit a.ts (+0 \u22120)");
  });
});

describe("receiptText", () => {
  it("says what was decided and about what", () => {
    expect(receiptText({ kind: "bash", command: "npm test" }, "allowed")).toBe("Allowed \u00b7 npm test");
    expect(receiptText({ kind: "bash", command: "rm -rf build" }, "denied")).toBe("Denied \u00b7 rm -rf build");
    expect(receiptText({ kind: "edit", path: "src/auth.ts", add: 1, del: 1, hunk: [] }, "allowed")).toBe("Allowed \u00b7 src/auth.ts");
    expect(receiptText({ kind: "fetch", url: "https://api.github.com/x" }, "allowed")).toBe("Allowed \u00b7 api.github.com");
    expect(receiptText({ kind: "mcp", server: "github", tool: "create_pr", args: {} }, "allowed")).toBe("Allowed \u00b7 create_pr");
  });
  it("uses only the first line of a command, cut to 40 characters", () => {
    const long = "npm run build -- --mode production --sourcemap --minify --report";
    expect(receiptText({ kind: "bash", command: `${long}\nsecond` }, "allowed")).toBe(`Allowed \u00b7 ${long.slice(0, 39)}\u2026`);
  });
});
