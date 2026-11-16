import { describe, expect, it } from "vitest";
import { releaseNotes } from "./notes.mjs";

const SHA = "AB".repeat(32);

describe("the release notes", () => {
  const notes = releaseNotes({ version: "0.1.0-alpha.1", sha256: SHA });

  it("carry the commands, the warning and the checksum", () => {
    for (const text of [
      "winget install Doofess.Marshell",
      "winget upgrade Doofess.Marshell",
      "winget uninstall Doofess.Marshell",
      "winget settings --enable LocalManifestFiles",
      "SmartScreen",
      "%USERPROFILE%\\.marshell",
      "winget-manifest-0.1.0-alpha.1.zip",
      "Marshell_0.1.0-alpha.1_x64-setup.exe",
      SHA,
    ]) {
      expect(notes, text).toContain(text);
    }
  });

  it("are plain ASCII, so the workflow's shell pipe can't garble them", () => {
    expect(notes).toMatch(/^[\x09\x0a\x20-\x7e]*$/);
  });

  it("say how a friend who installed from the release page updates and uninstalls", () => {
    expect(notes).toContain("winget uninstall --manifest");
    expect(notes).toContain("install the new version from its release page the same way");
  });

  it("say what an uninstall closes and what it leaves", () => {
    for (const text of [
      "close Marshell and the sessions running in it",
      "QWEN_HOME",
      "%LOCALAPPDATA%\\dev.marshell.app",
      "%USERPROFILE%\\.marshell\\bin\\marshell.exe uninstall",
    ]) {
      expect(notes, text).toContain(text);
    }
  });

  it("say the third-party notices aren't complete yet", () => {
    expect(notes).toContain("THIRD_PARTY_NOTICES.md");
    expect(notes).toContain("not all of them yet: the complete list comes before v1");
    expect(notes).toContain("`Cargo.lock` and `pnpm-lock.yaml`");
  });

  it("name no other version", () => {
    const other = releaseNotes({ version: "0.1.0-alpha.2", sha256: SHA });
    expect(other).toContain("Marshell_0.1.0-alpha.2_x64-setup.exe");
    expect(other).not.toContain("alpha.1");
  });
});
