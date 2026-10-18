import { describe, expect, it } from "vitest";
// @ts-expect-error plain .mjs script without types
import { scrub } from "./scrub-fixtures.mjs";

const opts = { home: "C:\\Users\\Home Office", user: "Home Office" };

describe("scrub-fixtures", () => {
  it("replaces every spelling of the home path", () => {
    const input = [
      "C:\\\\Users\\\\Home Office\\\\.claude",
      "C:\\Users\\Home Office\\.claude",
      "C:/Users/Home Office/x",
      "/c/Users/Home Office/y",
    ].join("\n");
    expect(scrub(input, opts)).toBe(
      [
        "C:\\\\Users\\\\user\\\\.claude",
        "C:\\Users\\user\\.claude",
        "C:/Users/user/x",
        "/c/Users/user/y",
      ].join("\n"),
    );
  });

  it("replaces the project slug form", () => {
    expect(scrub("C--Users-Home-Office-AppData", opts)).toBe("C--Users-user-AppData");
  });

  it("maps the same UUID to the same placeholder", () => {
    const a = "3f2b8c1e-1111-4222-8333-444455556666";
    const b = "aaaaaaaa-1111-4222-8333-444455556666";
    const state = { uuids: new Map() };
    const one = scrub(`${a} ${b} ${a}`, opts, state);
    expect(one).toBe(
      "00000000-0000-4000-8000-000000000001 00000000-0000-4000-8000-000000000002 00000000-0000-4000-8000-000000000001",
    );
    // A second file of the same run keeps the mapping and continues the counter.
    const c = "bbbbbbbb-1111-4222-8333-444455556666";
    expect(scrub(`${b} ${c}`, opts, state)).toBe(
      "00000000-0000-4000-8000-000000000002 00000000-0000-4000-8000-000000000003",
    );
  });

  it("replaces e-mail addresses", () => {
    expect(scrub("mail someone.else+tag@mail.example.org now", opts)).toBe(
      "mail user@example.com now",
    );
  });

  it("leaves other text alone", () => {
    const text = '{"type":"Stop","n":3,"path":"/tmp/x"}';
    expect(scrub(text, opts)).toBe(text);
  });
});
