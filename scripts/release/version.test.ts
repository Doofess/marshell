import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
// @ts-expect-error plain .mjs script without types
import { installerName, installerUrl, stamp, versionFromTag } from "./version.mjs";

const read = (p: string) => readFileSync(new URL(`../../${p}`, import.meta.url), "utf8");
const files = () => ({
  cargoToml: read("Cargo.toml"),
  tauriConf: read("src-tauri/tauri.conf.json"),
  packageJson: read("package.json"),
});

describe("version", () => {
  it("an alpha tag gives its version", () => {
    expect(versionFromTag("v0.1.0-alpha.1")).toBe("0.1.0-alpha.1");
    expect(versionFromTag("v0.2.3-alpha.12")).toBe("0.2.3-alpha.12");
  });

  it("anything else is refused, naming the tag", () => {
    for (const tag of [
      "v0.1.0",
      "v1",
      "0.1.0-alpha.1",
      "v0.1.0-alpha.0",
      "v0.1.0-alpha.1-x",
      "v0.1.0-beta.1",
      "",
    ]) {
      expect(() => versionFromTag(tag)).toThrow(tag);
      expect(() => versionFromTag(tag)).toThrow("v0.1.0-alpha.N");
    }
  });

  it("the stamp changes the three version lines and nothing else", () => {
    const input = files();
    const out = stamp(input, "0.1.0-alpha.7");
    for (const key of ["cargoToml", "tauriConf", "packageJson"] as const) {
      const a = input[key].split("\n");
      const b = out[key].split("\n");
      expect(b.length).toBe(a.length);
      const changed = b.filter((line: string, i: number) => line !== a[i]);
      expect(changed.length).toBe(1);
      expect(changed[0]).toContain("0.1.0-alpha.7");
    }
    expect(out.cargoToml).toContain('serde = { version = "1"');
  });

  it("a file without its version line is refused", () => {
    expect(() => stamp({ ...files(), cargoToml: "[workspace]\n" }, "0.1.0-alpha.7")).toThrow();
  });

  it("the installer's name and address", () => {
    expect(installerName("0.1.0-alpha.1")).toBe("Marshell_0.1.0-alpha.1_x64-setup.exe");
    expect(installerUrl("0.1.0-alpha.1")).toBe(
      "https://github.com/Doofess/marshell/releases/download/v0.1.0-alpha.1/Marshell_0.1.0-alpha.1_x64-setup.exe",
    );
  });
});
