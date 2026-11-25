import { describe, expect, it } from "vitest";
import type { DefaultFolder } from "../../generated/DefaultFolder";
import { HOME_FOLDER, browseStart, startFolderName } from "./startFolder";

const there: DefaultFolder = { path: "C:\\Users\\u\\Marshell", exists: true, chosen: false };
const gone: DefaultFolder = { path: "C:\\Users\\u\\Marshell", exists: false, chosen: false };

describe("startFolderName", () => {
  it("a project names itself", () => {
    expect(startFolderName("api", "C:\\Work\\other", there)).toBe("api");
  });
  it("a profile folder is named by its last part", () => {
    expect(startFolderName(undefined, "C:\\Work\\api", there)).toBe("api");
  });
  it("the default folder is named when the profile sets none", () => {
    expect(startFolderName(undefined, null, there)).toBe("Marshell");
    expect(startFolderName(undefined, "  ", there)).toBe("Marshell");
  });
  it("a missing default folder reads your home folder", () => {
    expect(startFolderName(undefined, null, gone)).toBe(HOME_FOLDER);
  });
  it("no default folder reads your home folder", () => {
    expect(startFolderName(undefined, null, null)).toBe("your home folder");
    expect(startFolderName(undefined, undefined, { path: null, exists: false, chosen: false })).toBe(HOME_FOLDER);
  });
});

describe("browseStart", () => {
  it("browseStart prefers the typed folder, then the project, then the default folder", () => {
    expect(browseStart("D:\\typed", "D:\\proj", there)).toBe("D:\\typed");
    expect(browseStart(null, "D:\\proj", there)).toBe("D:\\proj");
    expect(browseStart(null, null, there)).toBe("C:\\Users\\u\\Marshell");
  });
  it("browseStart skips a default folder that is not there", () => {
    expect(browseStart(null, null, gone)).toBeNull();
    expect(browseStart(null, null, null)).toBeNull();
  });
});
