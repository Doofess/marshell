import { describe, expect, it } from "vitest";
import type { FieldControl } from "../../generated/FieldControl";
import type { SettingValue } from "../../generated/SettingValue";
import { choiceOptions, draftValue, editSummary, formEdits, parseField, switchOptions } from "./companyEdit";

const num: FieldControl = { kind: "number", min: 1 };
const sw: FieldControl = { kind: "switch" };
const txt: FieldControl = { kind: "text" };
const ch: FieldControl & { kind: "choice" } = { kind: "choice", values: ["low", "high"] };
const sv = (key: string, value: string | null, control?: FieldControl, lock?: SettingValue["lock"]): SettingValue => ({ key, value, scope: "user", control, lock }) as SettingValue;

describe("parseField", () => {
  it("parses a number", () => {
    expect(parseField(num, "30")).toEqual({ ok: true, json: "30" });
    for (const bad of ["0", "1.5", "x"]) expect(parseField(num, bad)).toEqual({ ok: false, message: "Use a whole number of at least 1." });
  });
  it("gives null for an empty input of every kind", () => {
    for (const c of [num, sw, txt, ch]) expect(parseField(c, "")).toEqual({ ok: true, json: null });
  });
  it("parses a switch and text", () => {
    expect(parseField(sw, "on")).toEqual({ ok: true, json: "true" });
    expect(parseField(sw, "off")).toEqual({ ok: true, json: "false" });
    expect(parseField(txt, "opus")).toEqual({ ok: true, json: '"opus"' });
  });
});

describe("number control input", () => {
  it("rejects stray characters and trims spaces", () => {
    for (const bad of ["-", "1e"]) expect(parseField(num, bad)).toEqual({ ok: false, message: "Use a whole number of at least 1." });
    expect(parseField(num, " 45 ")).toEqual({ ok: true, json: "45" });
    expect(parseField(num, "")).toEqual({ ok: true, json: null });
  });
});

describe("switchOptions", () => {
  it("adds a value from the file after the three", () => {
    expect(switchOptions("yes").at(-1)).toEqual({ value: "yes", label: "yes (from the file)" });
    expect(switchOptions("on")).toHaveLength(3);
    expect(switchOptions("")).toHaveLength(3);
  });
  it("keeps an odd value as it is", () => {
    expect(draftValue(sv("c", "yes", sw))).toBe("yes");
    expect(draftValue(sv("c", "true", sw))).toBe("on");
    expect(draftValue(sv("c", "false", sw))).toBe("off");
  });
});

describe("untouched odd values", () => {
  it("plans no edit when the draft does not hold the key", () => {
    const odd = [sv("a", "yes", sw), sv("b", "abc", num), sv("c", "dracula", ch)];
    expect(formEdits(odd, {})).toEqual({ edits: [], errors: {} });
  });
});

describe("formEdits", () => {
  const curated = [sv("a", "low", ch), sv("b", "30", num), sv("c", "true", sw), sv("d", null, txt), sv("e", "hidden", txt, { kind: "hidden" }), sv("f", "custom:ocean", ch)];
  const untouched = () => Object.fromEntries(curated.map((v) => [v.key, draftValue(v)]));
  it("has no edit for an untouched draft", () => {
    expect(formEdits(curated, untouched())).toEqual({ edits: [], errors: {} });
  });
  it("has an edit for a changed key and omits value for a cleared one", () => {
    const { edits } = formEdits(curated, { ...untouched(), a: "high", b: "", c: "off", d: "x" });
    expect(edits).toEqual([{ key: "a", value: '"high"' }, { key: "b" }, { key: "c", value: "false" }, { key: "d", value: '"x"' }]);
    expect(edits[1]).not.toHaveProperty("value");
  });
  it("never edits a locked key and reports a bad number", () => {
    const r = formEdits(curated, { ...untouched(), e: "changed", b: "0" });
    expect(r.edits).toEqual([]);
    expect(r.errors).toEqual({ b: "Use a whole number of at least 1." });
  });
  it("leaves a value outside the choices alone until it changes", () => {
    expect(formEdits(curated, untouched()).edits).toEqual([]);
    expect(formEdits(curated, { ...untouched(), f: "low" }).edits).toEqual([{ key: "f", value: '"low"' }]);
  });
});

describe("choiceOptions and draftValue", () => {
  it("lists Not set, the values, then a current value from the file", () => {
    const withCustom = choiceOptions({ kind: "choice", values: ["acceptEdits", "xhigh", "low"] }, "custom:ocean");
    expect(withCustom).toEqual([
      { value: "", label: "Not set" },
      { value: "acceptEdits", label: "Accept edits" },
      { value: "xhigh", label: "Extra high" },
      { value: "low", label: "Low" },
      { value: "custom:ocean", label: "custom:ocean (from the file)" },
    ]);
    expect(choiceOptions(ch, "low")).toHaveLength(3);
  });
  it("round-trips each control", () => {
    expect(draftValue(sv("a", null, sw))).toBe("");
    expect(draftValue(sv("a", "true", sw))).toBe("on");
    expect(draftValue(sv("a", "false", sw))).toBe("off");
    expect(draftValue(sv("a", "30", num))).toBe("30");
    expect(draftValue(sv("a", "opus", txt))).toBe("opus");
    expect(parseField(num, draftValue(sv("a", "30", num)))).toEqual({ ok: true, json: "30" });
  });
  it("summarises edits", () => {
    expect(editSummary(0)).toBe("No changes");
    expect(editSummary(2)).toBe("2 changes");
  });
});
