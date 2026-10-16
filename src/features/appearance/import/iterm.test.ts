import { describe, expect, it } from "vitest";
import { parseItermColors } from "./iterm";

function color(r: number, g: number, b: number): string {
  return `<dict>
    <key>Alpha Component</key><real>1</real>
    <key>Blue Component</key><real>${b}</real>
    <key>Color Space</key><string>sRGB</string>
    <key>Green Component</key><real>${g}</real>
    <key>Red Component</key><real>${r}</real>
  </dict>`;
}

/** A plist with the 16 ANSI colours (minus any in `skip`) plus foreground and background. */
function plist(skip: number[] = [], extra = ""): string {
  let body = "";
  for (let i = 0; i < 16; i++) if (!skip.includes(i)) body += `<key>Ansi ${i} Color</key>${color(i / 15, 0, 0)}\n`;
  body += `<key>Foreground Color</key>${color(1, 1, 1)}\n<key>Background Color</key>${color(0, 0, 0)}\n${extra}`;
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>${body}</dict></plist>`;
}

describe("parseItermColors", () => {
  it("converts 0–1 components to hex", () => {
    const t = parseItermColors(plist([0], `<key>Ansi 0 Color</key>${color(1, 0.5, 0)}`));
    expect(t.colors.ansi[0]).toBe("#ff8000");
    expect(t.colors.ansi).toHaveLength(16);
    expect(t.colors.foreground).toBe("#ffffff");
    expect(t.colors.background).toBe("#000000");
    // Cursor and selection fall back to the foreground and mid grey.
    expect(t.colors.cursor).toBe("#ffffff");
    expect(t.colors.selection).toBe("#808080");
    expect(t.source).toBe("iterm2");
  });

  it("reads cursor and selection when present", () => {
    const t = parseItermColors(plist([], `<key>Cursor Color</key>${color(0, 1, 0)}<key>Selection Color</key>${color(0, 0, 1)}`));
    expect(t.colors.cursor).toBe("#00ff00");
    expect(t.colors.selection).toBe("#0000ff");
  });

  it("takes the name and id from the file name when given", () => {
    const t = parseItermColors(plist(), "Solarized Dark.itermcolors");
    expect(t.name).toBe("Solarized Dark");
    expect(t.id).toBe("iterm2:solarized-dark");
  });

  it("missing Ansi 7 Color is rejected naming it", () => {
    expect(() => parseItermColors(plist([7]))).toThrow(/Ansi 7 Color/);
  });

  it("rejects text that is not a plist", () => {
    expect(() => parseItermColors("hello")).toThrow(/plist/i);
  });
});
