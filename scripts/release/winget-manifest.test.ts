import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
// @ts-expect-error plain .mjs script without types
import { installerUrl } from "./version.mjs";
// @ts-expect-error plain .mjs script without types
import { FILE_IN_USE_EXIT, manifests } from "./winget-manifest.mjs";

const version = "0.1.0-alpha.1";
const sha = "AB".repeat(32);
const releaseDate = "2026-11-15";
const make = () => manifests({ version, sha256: sha, releaseDate }) as Record<string, string>;

describe("winget manifest", () => {
  it("three files, named for the id", () => {
    expect(Object.keys(make()).sort()).toEqual([
      "Doofess.Marshell.installer.yaml",
      "Doofess.Marshell.locale.en-US.yaml",
      "Doofess.Marshell.yaml",
    ]);
  });

  it("the installer manifest is a per-user Nullsoft one", () => {
    const text = make()["Doofess.Marshell.installer.yaml"];
    expect(text).toContain("InstallerType: nullsoft");
    expect(text).toContain("Scope: user");
    expect(text).toContain("ProductCode: Marshell");
    // winget upgrade installs over the old version. With uninstallPrevious it would run the old uninstaller,
    // and that takes the hooks out.
    expect(text).toContain("UpgradeBehavior: install");
    expect(text).toContain(`InstallerUrl: ${installerUrl(version)}`);
    expect(text).toContain(`InstallerSha256: ${sha}`);
    expect(text).toContain("ReleaseDate: 2026-11-15");
  });

  // An update that stops because a program is still running exits with the code hooks.nsh names. winget has no
  // default return codes for NSIS installers, so without this line it would print only the bare exit code.
  it("a stopped update is winget's file-in-use", () => {
    const lines = make()["Doofess.Marshell.installer.yaml"].split("\n");
    const at = lines.indexOf("UpgradeBehavior: install");
    expect(at).toBeGreaterThan(-1);
    expect(lines.slice(at + 1, at + 4)).toEqual([
      "ExpectedReturnCodes:",
      "- InstallerReturnCode: 32",
      "  ReturnResponse: fileInUse",
    ]);
    const hooks = readFileSync(new URL("../../src-tauri/windows/hooks.nsh", import.meta.url), "utf8");
    const held = /^!define MARSHELL_HELD_EXIT (\d+)$/m.exec(hooks);
    expect(held).not.toBeNull();
    expect(FILE_IN_USE_EXIT).toBe(Number(held?.[1]));
  });

  it("the locale manifest matches Add/Remove Programs", () => {
    const text = make()["Doofess.Marshell.locale.en-US.yaml"];
    expect(text).toContain("Publisher: Doofess");
    expect(text).toContain("PackageName: Marshell");
  });

  it("every file names the id, the version and the manifest version", () => {
    for (const text of Object.values(make())) {
      expect(text).toContain("PackageIdentifier: Doofess.Marshell");
      expect(text).toContain(`PackageVersion: ${version}`);
      expect(text).toContain("ManifestVersion: 1.10.0");
    }
  });

  it("a hash that isn't 64 upper-case hex characters, or a date that isn't a date, is refused", () => {
    expect(() => manifests({ version, sha256: sha.toLowerCase(), releaseDate })).toThrow();
    expect(() => manifests({ version, sha256: sha.slice(1), releaseDate })).toThrow();
    expect(() => manifests({ version, sha256: sha, releaseDate: "2026-11-5" })).toThrow();
  });
});
