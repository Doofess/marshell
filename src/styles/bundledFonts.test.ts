import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const dir = join(__dirname, "..", "assets", "fonts");
const css = readFileSync(join(__dirname, "fonts.css"), "utf8");
const notices = readFileSync(join(__dirname, "..", "..", "THIRD_PARTY_NOTICES.md"), "utf8");

const HASHES: Record<string, string> = {
  "CascadiaMono-Regular.woff2": "abbf67cceb200508c9590ac9b75b37dd2211f58ea56279f1fa6ec87265f9d713",
  "CascadiaMono-Bold.woff2": "d84776a2893f382b96c8bfe085d438dc07f2fdd98210fa9f2d3ac62d82a7aef4",
  "AtkinsonHyperlegibleMono-Regular.woff2": "3b916de5c5247c5fa9736aad6c2673f1b0b0381ab306dcddfcee4a184a842a51",
  "AtkinsonHyperlegibleMono-Bold.woff2": "039936edaa268366426a7f0ca8014da637b8bf8da24859afbd85a5b313e9315e",
  "JetBrainsMono-Regular.woff2": "a9cb1cd82332b23a47e3a1239d25d13c86d16c4220695e34b243effa999f45f2",
  "JetBrainsMono-Bold.woff2": "c503cc5ec5f8b2c7666b7ecda1adf44bd45f2e6579b2eba0fc292150416588a2",
};
const FAMILIES = ["JetBrains Mono", "Cascadia Mono", "Atkinson Hyperlegible Mono"];

function faces(): { family: string; weight: number; file: string }[] {
  const pick = (body: string, re: RegExp) => re.exec(body)?.[1] ?? "";
  return [...css.matchAll(/@font-face\s*\{([^}]*)\}/g)].map((m) => ({
    family: pick(m[1] ?? "", /font-family:\s*"([^"]+)"/),
    weight: Number(pick(m[1] ?? "", /font-weight:\s*(\d+)/)),
    file: pick(m[1] ?? "", /url\("\.\.\/assets\/fonts\/([^"]+)"\)/),
  }));
}

describe("bundled fonts", () => {
  const files = readdirSync(dir).filter((f) => f.endsWith(".woff2"));
  it("every font file has an @font-face at weight 400 or 700", () => {
    for (const f of files) {
      const face = faces().find((x) => x.file === f);
      expect(face, f).toBeDefined();
      expect([400, 700]).toContain(face!.weight);
    }
  });
  it("each family has both weights", () => {
    for (const fam of FAMILIES) {
      expect(faces().filter((x) => x.family === fam).map((x) => x.weight).sort(), fam).toEqual([400, 700]);
    }
  });
  it("each family is in THIRD_PARTY_NOTICES.md beside the SIL Open Font License", () => {
    for (const fam of FAMILIES) {
      const i = notices.indexOf(`## ${fam}`);
      expect(i, fam).toBeGreaterThanOrEqual(0);
      const next = notices.indexOf("\n## ", i + 1);
      expect(notices.slice(i, next < 0 ? undefined : next), fam).toContain("SIL Open Font License");
    }
  });
  it("each file is byte-identical to its release", () => {
    expect([...files].sort()).toEqual(Object.keys(HASHES).sort());
    for (const f of files) expect(createHash("sha256").update(readFileSync(join(dir, f))).digest("hex"), f).toBe(HASHES[f]);
  });
});
