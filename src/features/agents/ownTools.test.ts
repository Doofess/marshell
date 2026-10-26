import { describe, expect, it } from "vitest";
import type { ToolProfile } from "../../generated/ToolProfile";
import { ApiError } from "../../lib/api/client";
import { blankDraft, CONFLICT_TEXT, draftOf, escapeCancels, outcomeOf, placeholders, profileOf, refusalOf, slug } from "./ownTools";

const full: ToolProfile = {
  id: "my-tool",
  name: "My Tool",
  commands: ["mytool", "mt"],
  scripts: ["bin/mytool.js"],
  launcher: true,
  needs_you: [
    { pattern: "^Proceed\\?", phrase: "wants you to confirm", kind: "text" },
    { pattern: "open (https?://\\S+)", phrase: "wants you to sign in", kind: "browser" },
  ],
  ports: [{ pattern: "listening on (\\S+)" }],
  quiet_ports: ["auth login"],
  presets: [{ id: "deploy", name: "Deploy", program: "mytool", args: ["deploy", "{target}", "--ns", "{ns}"], fields: [{ id: "target", label: "Target" }, { id: "ns", label: "Namespace" }] }],
};

describe("ownTools drafts", () => {
  it("round-trips a profile with every field, including the hidden ones", () => {
    expect(profileOf(draftOf(full))).toEqual(full);
  });
  it("sends an unedited entry back exactly as shown", () => {
    const odd = { ...full, id: "Odd_Id", presets: [{ id: "x", name: "X", program: "p", args: ["", "a b"], fields: [{ id: "stale", label: "Stale" }] }] };
    expect(profileOf(draftOf(odd), odd)).toBe(odd);
  });
  it("reads {target} and {ns} in args as the fields target and ns", () => {
    expect(placeholders("deploy\n{target}\n--ns\n{ns}\n{target}")).toEqual(["target", "ns"]);
    expect(placeholders("a {not whole}\n")).toEqual([]);
  });
  it("keeps a label edit when the args change", () => {
    const d = draftOf(full);
    const p0 = d.presets[0]!;
    const edited = { ...d, presets: [{ ...p0, labels: { ...p0.labels, target: "Where to" }, args: `${p0.args}\n--dry-run` }] };
    const out = profileOf(edited);
    expect(out.presets[0]!.args.at(-1)).toBe("--dry-run");
    expect(out.presets[0]!.fields).toEqual([{ id: "target", label: "Where to" }, { id: "ns", label: "Namespace" }]);
  });
  it("labels a new placeholder with its id until the person names it", () => {
    const out = profileOf({ ...blankDraft(), name: "T", commands: "t", presets: [{ id: "", name: "Go", program: "t", args: "{where}", labels: {} }] });
    expect(out.presets[0]!.fields).toEqual([{ id: "where", label: "where" }]);
    expect(out.presets[0]!.id).toBe("go");
  });
  it("makes the id a lowercase slug", () => {
    expect(slug("My Tool!")).toBe("my-tool");
    expect(profileOf({ ...blankDraft(), name: "My Tool!", commands: "a, b" }).id).toBe("my-tool");
    expect(profileOf({ ...blankDraft(), name: "My Tool!", commands: "a, b" }).commands).toEqual(["a", "b"]);
  });
});

describe("ownTools outcomes", () => {
  it("reads a 409 as the reload text and a 422 as the problems", () => {
    expect(outcomeOf(new ApiError(409, "x"))).toEqual({ kind: "conflict", text: CONFLICT_TEXT });
    expect(CONFLICT_TEXT).toBe("tools.json changed since you opened it. Reload to see it.");
    const problems = [{ index: 1, id: "a", reason: "r" }];
    expect(outcomeOf(new ApiError(422, JSON.stringify(problems)))).toEqual({ kind: "refused", problems });
    expect(outcomeOf(new ApiError(500, "no")).kind).toBe("failed");
  });
  it("puts a refusal on the entry it names, mapping a removal's shifted indexes back", () => {
    const ps = [{ index: 2, id: "c", reason: "bad" }];
    expect(refusalOf(ps, { replace: 2 }, 3)).toEqual({ entries: ps, form: undefined });
    expect(refusalOf(ps, { add: true }, 2)).toEqual({ entries: [], form: "bad" });
    expect(refusalOf(ps, { remove: 0 }, 4).entries).toEqual([{ index: 3, id: "c", reason: "bad" }]);
  });
  it("Esc on a question cancels and prevents the default", () => {
    let prevented = false;
    let cancelled = false;
    escapeCancels({ key: "Escape", preventDefault: () => (prevented = true) }, () => (cancelled = true));
    expect([prevented, cancelled]).toEqual([true, true]);
    cancelled = false;
    escapeCancels({ key: "a", preventDefault: () => {} }, () => (cancelled = true));
    expect(cancelled).toBe(false);
  });
});
