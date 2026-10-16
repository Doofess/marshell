import { describe, expect, it } from "vitest";
import type { Profile } from "../../generated/Profile";
import type { ProfilesDoc } from "../../generated/ProfilesDoc";
import { changeCount, commandRoundTrips, deleteProfile, duplicateEnvKeys, duplicateProfile, forSave, joinCommand, makeDefault, profileKind, resetProfile, splitCommand, updateProfile } from "./profilesDoc";

const profile = (id: string, patch: Partial<Profile> = {}): Profile => ({ id, name: id.toUpperCase(), command: [`${id}.exe`], family: "cmd", cwd: null, env: [], shell_integration: true, detected: false, ...patch });
const FOUND: Profile[] = [profile("pwsh", { detected: true }), profile("cmd", { detected: true })];
const doc = (...extra: Profile[]): ProfilesDoc => ({ profiles: [...FOUND, ...extra], default_id: "pwsh" });

describe("command text", () => {
  it("splits on spaces and keeps a quoted part together", () => {
    expect(splitCommand('"C:\\Program Files\\Git\\bin\\bash.exe" -l')).toEqual(["C:\\Program Files\\Git\\bin\\bash.exe", "-l"]);
    expect(splitCommand("  pwsh   -NoLogo ")).toEqual(["pwsh", "-NoLogo"]);
    expect(splitCommand("")).toEqual([]);
    expect(splitCommand('a "" b')).toEqual(["a", "", "b"]);
    expect(splitCommand('say "hello there')).toEqual(["say", "hello there"]);
  });
  it("joins back, quoting only what has a space or is empty", () => {
    expect(joinCommand(["C:\\Program Files\\Git\\bin\\bash.exe", "-l"])).toBe('"C:\\Program Files\\Git\\bin\\bash.exe" -l');
    expect(joinCommand(["wsl.exe", "-d", "Ubuntu"])).toBe("wsl.exe -d Ubuntu");
    expect(joinCommand(["a", ""])).toBe('a ""');
  });
  it("round-trips", () => {
    for (const argv of [["C:\\Program Files\\PowerShell\\7\\pwsh.exe", "-NoLogo"], ["cmd.exe", "/k", "echo hi there"], ["x", ""]]) expect(splitCommand(joinCommand(argv))).toEqual(argv);
  });
});

describe("profile kind", () => {
  it("tells custom, detected and edited detected apart", () => {
    const edited = profile("cmd", { command: ["cmd.exe", "/k"] });
    expect(profileKind(FOUND[0]!, FOUND)).toBe("detected");
    expect(profileKind(edited, FOUND)).toBe("edited");
    expect(profileKind(profile("custom-1"), FOUND)).toBe("custom");
    // The server flag does not matter: an edited-back profile is the detected one again.
    expect(profileKind(profile("cmd", { detected: false }), FOUND)).toBe("detected");
  });
});

describe("editing the document", () => {
  it("updates one profile", () => {
    const d = updateProfile(doc(), "cmd", { command: ["cmd.exe", "/k"] });
    expect(d.profiles[1]!.command).toEqual(["cmd.exe", "/k"]);
    expect(d.profiles[0]).toEqual(FOUND[0]);
  });
  it("duplicates as custom-<n> with a name of its own", () => {
    let r = duplicateProfile(doc(), "pwsh");
    expect(r.id).toBe("custom-1");
    expect(r.doc.profiles.at(-1)).toMatchObject({ id: "custom-1", name: "PWSH copy", command: ["pwsh.exe"], detected: false });
    r = duplicateProfile(r.doc, "pwsh");
    expect(r.id).toBe("custom-2");
    expect(r.doc.profiles.at(-1)!.name).toBe("PWSH copy 2");
    // The lowest free number is reused.
    const gap = deleteProfile(r.doc, "custom-1", FOUND);
    expect(duplicateProfile(gap, "cmd").id).toBe("custom-1");
  });
  it("never deletes a detected profile, and moves the default off a deleted one", () => {
    expect(deleteProfile(doc(), "pwsh", FOUND)).toEqual(doc());
    const withCustom = makeDefault(doc(profile("custom-1")), "custom-1");
    const after = deleteProfile(withCustom, "custom-1", FOUND);
    expect(after.profiles.map((p) => p.id)).toEqual(["pwsh", "cmd"]);
    expect(after.default_id).toBe("pwsh");
  });
  it("resets a detected profile to the values detection found", () => {
    const edited = updateProfile(doc(), "cmd", { name: "Mine", env: [["A", "1"]] });
    expect(resetProfile(edited, "cmd", FOUND)).toEqual(doc());
    expect(resetProfile(doc(profile("custom-1")), "custom-1", FOUND)).toEqual(doc(profile("custom-1")));
  });
});

describe("saving", () => {
  it("drops env rows with no name", () => {
    const d = updateProfile(doc(), "cmd", { env: [["A", "1"], ["  ", "x"], ["", ""]] });
    expect(forSave(d).profiles[1]!.env).toEqual([["A", "1"]]);
  });
  it("trims the names of environment variables before they are sent", () => {
    const d = updateProfile(doc(), "cmd", { env: [["  A ", "1"]] });
    expect(forSave(d).profiles[1]!.env).toEqual([["A", "1"]]);
  });
  it("finds environment variables named twice (trimmed, any letter case, as Windows reads them)", () => {
    expect(duplicateEnvKeys([["A", "1"], ["B", "2"]])).toEqual([]);
    expect(duplicateEnvKeys([["Path", "1"], [" PATH ", "2"], ["B", ""]])).toEqual(["Path"]);
    expect(duplicateEnvKeys([["", "1"], ["  ", "2"]])).toEqual([]);
  });
  it("knows which commands the one-line field can write back unchanged", () => {
    expect(commandRoundTrips(["C:\\Program Files\\Git\\bin\\bash.exe", "-l"])).toBe(true);
    expect(commandRoundTrips([])).toBe(true);
    expect(commandRoundTrips(["sh", "-c", 'echo "hi"'])).toBe(false);
    expect(commandRoundTrips(['a"b'])).toBe(false);
  });
  it("counts what changed against the saved document", () => {
    expect(changeCount(doc(), doc())).toBe(0);
    expect(changeCount(doc(), updateProfile(doc(), "cmd", { name: "x" }))).toBe(1);
    expect(changeCount(doc(), makeDefault(doc(), "cmd"))).toBe(1);
    const added = duplicateProfile(doc(), "cmd").doc;
    expect(changeCount(doc(), added)).toBe(1);
    expect(changeCount(added, doc())).toBe(1);
  });
});
