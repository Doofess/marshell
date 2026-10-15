import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { COMMANDS } from "../../lib/keymap";
import { PALETTE_SESSIONS } from "./fixtures";
import { Palette } from "./Palette";

const html = (query = "", os: "windows" | "mac" | "linux" = "windows", activeIndex = 0) => renderToStaticMarkup(<Palette view={{ query, activeIndex }} os={os} />);
const withSessions = (query = "", os: "windows" | "mac" | "linux" = "windows", activeIndex = 0) =>
  renderToStaticMarkup(<Palette view={{ query, activeIndex }} sessions={PALETTE_SESSIONS} os={os} />);
const count = (s: string, re: RegExp) => (s.match(re) ?? []).length;
const options = (h: string) => count(h, /role="option"/g);

describe("Palette: empty", () => {
  const h = html();
  it("is a modal dialog with a combobox over a listbox", () => {
    expect(h).toContain('role="dialog"');
    expect(h).toContain('aria-modal="true"');
    expect(h).toContain('role="combobox"');
    expect(h).toContain('role="listbox"');
  });
  it("lists every command, grouped, in the keymap's order", () => {
    for (const c of COMMANDS) expect(h, c.id).toContain(c.label.replace(/'/g, "&#x27;"));
    for (const g of ["Sessions", "View", "Tools"]) expect(h).toContain(g);
  });
  it("shows each command's shortcut, rebindable, per OS", () => {
    expect(h).toContain("Ctrl+Shift+T");
    expect(html("", "mac")).toContain("⌘T");
    expect(h).toContain("Rebind in Settings");
  });
  it("gives every option an id and points the input at the active one", () => {
    expect(h).toMatch(/aria-activedescendant="[^"]*-opt-0"/);
    expect(options(h)).toBe(COMMANDS.length);
  });
  it("shows commands that have no shortcut without a gap or a dash", () => {
    expect(h).toContain("Open Doctor");
  });
});

describe("Palette: filtering", () => {
  it("ranks the best match first and highlights it", () => {
    const h = html("split");
    expect(h).toContain("Split</mark>");
    expect(h).toContain("<mark");
    expect(options(h)).toBeLessThan(COMMANDS.length);
  });
  it("drops the group headings while filtering", () => {
    expect(html("split")).not.toContain("palette__group");
  });
  it("says what happened when nothing matches", () => {
    const h = html("qqqq");
    expect(h).toContain("No command matches");
    expect(h).toContain('role="status"');
    expect(h).not.toContain('role="listbox"');
    expect(h).not.toContain("aria-controls");
    expect(h).toContain('aria-expanded="false"');
  });
  it("renders hostile text as text", () => {
    expect(html("<img src=x onerror=alert(1)>")).not.toContain("<img");
  });
});

describe("Palette: open sessions, each with its number", () => {
  const h = withSessions();
  it("lists every open session first, under its own heading, in number order", () => {
    expect(h).toContain("Open sessions");
    const numbers = [...h.matchAll(/class="palette__num"[^>]*>(\d+)</g)].map((m) => Number(m[1]));
    expect(numbers).toEqual(PALETTE_SESSIONS.map((s) => s.number).sort((a, b) => a - b));
    expect(h.indexOf("Open sessions")).toBeLessThan(h.indexOf("Start a session"));
  });
  it("shows the number, the mark, the name and where it runs on every session, AI agent or manual CLI", () => {
    for (const s of PALETTE_SESSIONS) {
      expect(h, s.name).toContain(s.name);
      expect(h, s.name).toContain(`Go to session ${s.number}, ${s.name}`);
    }
    expect(h).toContain('data-agent="powershell"');
    expect(h).toContain('data-agent="claude"');
  });
  it("leaves out the nine Go to session N commands: the session's own row is that command", () => {
    expect(h).not.toMatch(/<span class="palette__label"[^>]*><span>Go to session \d<\/span>/);
    expect(options(h)).toBe(PALETTE_SESSIONS.length + COMMANDS.filter((c) => !c.id.startsWith("focus-session-")).length);
    expect(h).toContain("Go to session…");
  });
  it("shows Ctrl+Shift+N for sessions 1 to 9 and nothing for 10 and up, per OS", () => {
    expect(h).toContain("Ctrl+Shift+6");
    expect(withSessions("", "mac")).toContain("⌘6");
    const twelve = PALETTE_SESSIONS.find((s) => s.number === 12)!;
    expect(h).toContain(`Go to session 12, ${twelve.name}`);
    expect(h).not.toContain("Ctrl+12");
  });
  it("gives every option an id and points the input at the active one", () => {
    const target = /aria-activedescendant="([^"]*-opt-0)"/.exec(h)?.[1];
    expect(target).toBeTruthy();
    expect(h).toContain(`id="${target}"`);
  });
  it("asks for a command or a session number", () => {
    expect(h).toContain('placeholder="Type a command, or a session number"');
  });
});

describe("Palette: say session 6 and it knows which terminal you mean", () => {
  it("puts the session with that number first for 6, #6 and session 6", () => {
    for (const q of ["6", "#6", "session 6", "go to session 6"]) {
      const h = withSessions(q);
      const first = /id="[^"]*-opt-0"[^>]*aria-label="([^"]*)"|aria-label="([^"]*)"[^>]*id="[^"]*-opt-0"/.exec(h);
      expect(first?.[1] ?? first?.[2], q).toContain("Go to session 6,");
    }
  });
  it("shows only sessions for a number, never a command that happens to contain it", () => {
    const h = withSessions("4");
    expect(h).not.toContain("Split: add a pane");
  });
  it("highlights nothing in the number's own row: the number is the match", () => {
    expect(withSessions("6")).toContain('data-exact="true"');
  });
  it("says which numbers exist when nobody has the one asked for", () => {
    const h = withSessions("#4");
    expect(h).toContain("No session 4");
    expect(h).toContain("1, 2, 3, 5, 6, 7, 8, 9, 12");
    expect(h).toContain('role="status"');
  });
  it("finds a session by name or folder, highlighted", () => {
    const h = withSessions("bill");
    expect(h).toContain("<mark");
    expect(h).toContain("Go to session 2, billing");
  });
  it("still finds commands, below the sessions", () => {
    const h = withSessions("split");
    expect(h).toContain("Split</mark>");
    expect(h).not.toContain("palette__group");
  });
  it("renders hostile text as text", () => {
    expect(withSessions("<img src=x onerror=alert(1)>")).not.toContain("<img");
  });
  it("says session or command when nothing matches", () => {
    expect(withSessions("qqqq")).toContain("No session or command matches");
  });
});

describe("Palette: hygiene", () => {
  it("never puts a literal colour in markup", () => {
    expect(html()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
    expect(withSessions()).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(|oklch\(/i);
  });
});

describe("Palette: odd session numbers and a stale active row", () => {
  const odd = (n: number) => [{ ...PALETTE_SESSIONS[0]!, id: `odd${n}`, number: n }];
  for (const n of [0, -1, 2.5, 10, 12]) {
    it(`lists a session numbered ${n} without throwing, and gives it no Ctrl+digit`, () => {
      const h = renderToStaticMarkup(<Palette view={{ query: "", activeIndex: 0 }} sessions={odd(n)} />);
      expect(h).toContain(`Go to session ${n},`);
      expect(h).not.toContain("<kbd class=\"palette__kbd\">Ctrl+Shift+" + n);
    });
  }
  it("clamps an active index that no longer exists, so the input never points at a missing option and one row is selected", () => {
    for (const idx of [999, -3, 1.5, Number.NaN]) {
      const h = withSessions("", "windows", idx);
      const target = /aria-activedescendant="([^"]*)"/.exec(h)?.[1];
      expect(target, String(idx)).toBeTruthy();
      expect(h, String(idx)).toContain(`id="${target}"`);
      expect(count(h, /aria-selected="true"/g), String(idx)).toBe(1);
    }
  });
  it("uses ids that are unique when two palettes are on one page (the review page shows many)", () => {
    const h = renderToStaticMarkup(
      <>
        <Palette view={{ query: "", activeIndex: 0 }} />
        <Palette view={{ query: "", activeIndex: 0 }} />
      </>,
    );
    const ids = [...h.matchAll(/ id="([^"]+)"/g)].map((m) => m[1]);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("Palette: commands", () => {
  it("lists the commands it is given and no others", () => {
    const some = COMMANDS.filter((c) => c.id === "launcher" || c.id === "profiles");
    const h = renderToStaticMarkup(<Palette view={{ query: "", activeIndex: 0 }} commands={some} os="windows" />);
    expect(options(h)).toBe(2);
    expect(h).toContain("Shell profiles");
    expect(h).not.toContain("Open Doctor");
    // A search only finds among them.
    expect(renderToStaticMarkup(<Palette view={{ query: "doctor", activeIndex: 0 }} commands={some} os="windows" />)).toContain("No command matches");
  });
  it("lists every command when none are given", () => {
    expect(options(html())).toBe(COMMANDS.length);
  });
});
