import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { PreviewPane, type PreviewPaneProps } from "../preview/Preview";
import { LayoutContext, type ColumnInputs } from "../window/paneColumn";
import { SidePane, tabTitle, type SidePaneProps, type StripTab } from "./SidePane";

const noop = () => {};
const PATH1 = "C:\\w\\a.md";
const tabs: StripTab[] = [
  { id: "t1", title: "a.md", path: "C:\\w\\a.md", kind: "text", openedBy: null, unseen: false },
  { id: "t2", title: "b.rs", path: "C:\\w\\b.rs", kind: "text", openedBy: null, unseen: false },
];
const html = (more: Partial<SidePaneProps> = {}) =>
  renderToStaticMarkup(
    <SidePane
      panel="files"
      tabs={tabs}
      activeId="t2"
      plus={{ label: "Open a file", menu: false }}
      expand={{ on: false, others: true }}
      hideKeys="Ctrl+Shift+A"
      onSelect={noop}
      onClose={noop}
      onMove={noop}
      onHide={noop}
      onPlus={noop}
      onExpand={noop}
      {...more}
    >
      {more.children ?? <p>body</p>}
    </SidePane>,
  );

describe("SidePane: a panel with no tabs", () => {
  const out = () => html({ panel: "changes", tabs: [], activeId: null, plus: undefined, title: "4 files · +120 −18", hideKeys: "Ctrl+Shift+H" });
  it("a tabless panel shows its title and no plus", () => {
    const h = out();
    expect(h).toContain('<span class="side-pane__title" role="heading" aria-level="2">4 files · +120 −18</span>');
    expect(h).not.toContain('role="tablist"');
    expect(h).not.toContain("data-pane-plus");
    expect(h).toContain('aria-label="Changes"');
    expect(h).toContain('aria-label="Hide Changes"');
  });
  it("a panel with no title keeps the tablist", () => {
    expect(html()).toContain('role="tablist"');
    expect(html()).toContain("data-pane-plus");
  });
});

describe("SidePane", () => {
  it("is its panel's section and a navigation stop", () => {
    const out = html();
    expect(out).toContain("<section");
    expect(out).toContain('aria-label="Files"');
    expect(out).toContain('data-nav-side="files"');
    expect(out).not.toContain("data-drop-pane");
    expect(out).toContain("<p>body</p>");
  });
  it("names its tablist after the panel", () => {
    expect(html()).toContain('role="tablist" aria-label="Files tabs"');
    expect(html({ panel: "browser" })).toContain('aria-label="Browser tabs"');
  });
  it("has a tablist of tabs wired to their panels, the path as the title", () => {
    const out = html();
    expect(out).toContain('role="tablist"');
    expect(out.match(/<button[^>]*role="tab"/g)).toHaveLength(2);
    expect(out).toContain('id="pane-tab-t1"');
    expect(out).toContain('aria-controls="pane-panel-t1"');
    expect(out).toContain('title="C:\\w\\b.rs"');
    expect(out).toMatch(/id="pane-tab-t2"[^>]*aria-selected="true"|aria-selected="true"[^>]*id="pane-tab-t2"/);
  });
  it("makes only the active tab a tab stop", () => {
    const out = html();
    const own = (id: string) => out.match(new RegExp(`<button[^>]*id="pane-tab-${id}"[^>]*>`))![0];
    expect(own("t2")).toContain('tabindex="0"');
    expect(own("t1")).toContain('tabindex="-1"');
  });
  it("gives each tab a sibling close button outside the tab", () => {
    const out = html();
    expect(out).toMatch(/<button[^>]*aria-label="Close a\.md"[^>]*>/);
    expect(out.match(/<button[^>]*aria-label="Close a\.md"[^>]*>/)![0]).toContain('tabindex="-1"');
    expect(out).not.toMatch(/<button[^>]*role="tab"[^>]*>(?:(?!<\/button>).)*<button/);
  });
  const btn = (out: string, label: string) => out.match(new RegExp(`<button[^>]*aria-label="${label}"[^>]*>`))![0];
  const order = (out: string, ...labels: string[]) => labels.map((l) => out.indexOf(`aria-label="${l}"`));
  it("ends with +, the menu when it has one, Expand and Hide, in that order", () => {
    const out = html({ panel: "browser", plus: { label: "New tab", menu: true }, menu: { label: "Browser menu" }, onMenu: noop });
    const at = order(out, "Close b.rs", "New tab", "Browser menu", "Expand Browser", "Hide Browser");
    expect([...at].sort((x, y) => x - y)).toEqual(at);
    expect(at[0]).toBeGreaterThan(-1);
    expect(btn(out, "Browser menu")).toContain('aria-haspopup="menu"');
    expect(btn(out, "Browser menu")).toContain('title="Browser menu"');
    expect(btn(out, "New tab")).toContain("data-pane-plus");
  });
  it("the Files panel has no menu button", () => {
    const out = html();
    expect(out).not.toContain("menu");
    expect(out).toContain('aria-label="Open a file"');
  });
  it("+ opens a menu only when told to", () => {
    expect(btn(html({ plus: { label: "New tab", menu: true } }), "New tab")).toContain('aria-haspopup="menu"');
    expect(btn(html(), "Open a file")).not.toContain("aria-haspopup");
  });
  it("a + that can't be used is aria-disabled and says why", () => {
    const b = btn(html({ plus: { label: "Open a file", menu: false, disabled: "No session is open" } }), "Open a file");
    expect(b).toContain('aria-disabled="true"');
    expect(b).not.toMatch(/\sdisabled/);
    expect(b).toContain('title="Open a file: No session is open"');
    expect(btn(html(), "Open a file")).not.toContain("aria-disabled");
  });
  it("Expand is pressed while its panel is expanded, and its tooltip says what pressing it brings back", () => {
    const on = btn(html({ expand: { on: true, others: true } }), "Expand Files");
    expect(on).toContain('aria-pressed="true"');
    expect(on).toContain('title="Show the terminal and the other panels"');
    const off = btn(html(), "Expand Files");
    expect(off).toContain('aria-pressed="false"');
    expect(off).toContain('title="Expand Files"');
  });
  it("Expand with no terminal beside and no other panel is aria-disabled and says why", () => {
    const layout = { inputs: {} as ColumnInputs, moving: false, beside: false };
    const out = renderToStaticMarkup(
      <LayoutContext.Provider value={layout}>
        <SidePane panel="files" tabs={tabs} activeId="t2" expand={{ on: false, others: false }} hideKeys="Ctrl+Shift+A" onSelect={noop} onClose={noop} onMove={noop} onHide={noop} onExpand={noop}>
          <p>body</p>
        </SidePane>
      </LayoutContext.Provider>,
    );
    const b = btn(out, "Expand Files");
    expect(b).toContain('aria-disabled="true"');
    expect(b).not.toMatch(/\sdisabled/);
    expect(b).toContain('title="No other panel is shown"');
    // With no provider the terminal is taken to be beside, so a lone panel can still expand.
    const alone = btn(html({ expand: { on: false, others: false } }), "Expand Files");
    expect(alone).not.toContain("aria-disabled");
    expect(alone).toContain('title="Expand Files"');
  });
  it("names each Expand button after its panel and draws Hide as a bar", () => {
    expect(html({ panel: "browser" })).toContain('aria-label="Expand Browser"');
    expect(html({ panel: "files" })).toContain('title="Expand Files"');
    expect(btn(html({ panel: "browser", expand: { on: true, others: true } }), "Expand Browser")).toContain('title="Show the terminal and the other panels"');
    const hide = html({ panel: "browser" }).match(/<button[^>]*aria-label="Hide Browser"[^>]*>.*?<\/button>/)![0];
    expect(hide).toContain('d="M3 6h6"');
    expect(hide).not.toContain("M3 3l6 6M9 3 3 9");
  });
  it("Hide names the panel and its key", () => {
    expect(html()).toMatch(/aria-label="Hide Files" title="Hide Files \(Ctrl\+Shift\+A\)"/);
    expect(html({ panel: "browser" })).toContain('aria-label="Hide Browser" title="Hide Browser (Ctrl+Shift+A)"');
  });
  it("a web tab shows the globe and never an image", () => {
    const globe = html({ tabs: [{ id: "b1", title: "Example.com", path: "https://example.com/", kind: "web", openedBy: null, unseen: false }], activeId: "b1" });
    expect(globe).not.toContain("<img");
    expect(globe).toMatch(/<svg[^>]*class="side-pane__icon"[^>]*data-kind="web"[^>]*aria-hidden="true"|<svg(?=[^>]*data-kind="web")(?=[^>]*class="side-pane__icon")(?=[^>]*aria-hidden="true")/);
    // The icon sits inside the tab button, before the title.
    expect(globe.indexOf("side-pane__icon")).toBeLessThan(globe.indexOf("side-pane__title"));
    expect(globe.indexOf('role="tab"')).toBeLessThan(globe.indexOf("side-pane__icon"));
    // A web title shortens at its end: it is not split.
    expect(globe).not.toContain("side-pane__name-head");
    expect(globe).toContain('<span class="side-pane__title">Example.com</span>');
  });
  it("text, pdf and image tabs each show their own icon", () => {
    for (const kind of ["text", "pdf", "image"] as const) {
      const out = html({ tabs: [{ id: "f1", title: "a.x", path: "a.x", kind, openedBy: null, unseen: false }], activeId: "f1" });
      expect(out).toMatch(new RegExp(`<svg(?=[^>]*data-kind="${kind}")(?=[^>]*class="side-pane__icon")`));
      expect(out).not.toContain("<img");
    }
  });
  it("a file tab's name is a head that shortens and a tail that keeps its extension", () => {
    const out = html({ tabs: [{ id: "f1", title: "sample-long-name.txt", path: "C:\\w\\sample-long-name.txt", kind: "text", openedBy: null, unseen: false }], activeId: "f1" });
    expect(out).toContain('<span class="side-pane__name-head">sample-long-name</span><span class="side-pane__name-tail">.txt</span>');
  });
  it("with no tabs shows no tablist, and still its buttons", () => {
    const out = html({ tabs: [], activeId: null });
    expect(out).not.toContain('role="tablist"');
    expect(out).toContain('aria-label="Open a file"');
    expect(out).toContain('aria-label="Expand Files"');
    expect(out).toContain('aria-label="Hide Files"');
  });

  it("a tab whose file can't be read still shows in the strip and closes", () => {
    const pane: PreviewPaneProps = {
      entry: { path: "C:\\w\\a.md", line: null, col: null, view: "rendered" },
      file: { phase: "error" },
      updated: false,
      announce: noop,
      refetch: noop,
      os: "windows",
      active: true,
      canBack: false,
      canForward: false,
      onBack: noop,
      onForward: noop,
      onOpen: noop,
      onView: noop,
      onEscape: noop,
      regionTarget: null,
      onRegion: () => Promise.resolve(),
    };
    const out = html({ tabs: [tabs[0]!], activeId: "t1", children: <PreviewPane {...pane} /> });
    expect(out).toContain('id="pane-tab-t1"');
    expect(out).toContain('aria-label="Close a.md"');
    expect(out).toContain("This file can&#x27;t be previewed.");
  });
});

describe("SidePane: the mark on a tab that marshell open added", () => {
  const marked = (openedBy: string | null, unseen: boolean): StripTab[] => [{ ...tabs[0]!, openedBy, unseen }, tabs[1]!];
  const tab1 = (out: string) => out.match(/<button[^>]*id="pane-tab-t1"[\s\S]*?<\/button>/)![0];
  it("an_unseen_tab_shows_its_agents_mark", () => {
    const t = tab1(html({ tabs: marked("codex", true) }));
    expect(t).toContain("agent-mark");
    expect(t).toContain("side-pane__mark");
    expect(t).not.toContain("side-pane__new\"");
    expect(t).toContain(`title="${PATH1} · Opened by Codex"`);
    expect(t).toMatch(/side-pane__new-word[^>]*>, new</);
    expect(t).toMatch(/<span class="side-pane__mark" aria-hidden="true"><svg/);
  });
  it("an_unseen_shell_tab_shows_a_plain_dot", () => {
    const t = tab1(html({ tabs: marked("shell", true) }));
    expect(t).toContain('class="side-pane__new"');
    expect(t).toContain("Opened with marshell open");
  });
  it("a_seen_tab_keeps_only_the_tooltip", () => {
    const t = tab1(html({ tabs: marked("claude", false) }));
    expect(t).toContain(`Opened by Claude Code`);
    expect(t).not.toContain("side-pane__new");
    expect(t).not.toContain("agent-mark");
    expect(t).not.toContain(", new");
  });
  it("an_unknown_agent_id_shows_the_plain_dot", () => {
    const t = tab1(html({ tabs: marked("my-agent", true) }));
    expect(t).toContain('class="side-pane__new"');
    expect(t).toContain("Opened by my-agent");
    expect(t).not.toContain("marshell open");
  });
  it("a_tab_nobody_opened_keeps_its_path_as_the_title", () => {
    expect(tab1(html({ tabs: marked(null, false) }))).toContain(`title="${PATH1}"`);
  });
});

describe("SidePane: the dot on a tab with unsaved edits", () => {
  const unsaved = (over: Partial<StripTab> = {}): StripTab[] => [{ ...tabs[0]!, unsaved: true, ...over }, tabs[1]!];
  const tab1 = (out: string) => out.match(/<button[^>]*id="pane-tab-t1"[\s\S]*?<\/button>/)![0];
  it("an_unsaved_tab_shows_the_dot_and_says_unsaved", () => {
    const t = tab1(html({ tabs: unsaved() }));
    expect(t).toMatch(/<span class="side-pane__unsaved" aria-hidden="true"><\/span>/);
    expect(t).toMatch(/side-pane__new-word[^>]*>, unsaved</);
    expect(t).toContain(`title="${PATH1} · Unsaved edits"`);
    // The dot sits in the opener's slot, before the type icon.
    expect(t.indexOf("side-pane__unsaved")).toBeLessThan(t.indexOf("side-pane__icon"));
    // A saved tab has none of it.
    const clean = tab1(html());
    expect(clean).not.toContain("side-pane__unsaved");
    expect(clean).not.toContain(", unsaved");
    expect(clean).not.toContain("Unsaved edits");
  });
  it("the_openers_mark_wins_over_the_dot", () => {
    const t = tab1(html({ tabs: unsaved({ openedBy: "codex", unseen: true }) }));
    expect(t).toContain("agent-mark");
    expect(t).not.toContain("side-pane__unsaved");
    // The name and the tooltip still tell both.
    expect(t).toMatch(/, new<\/span><span class="side-pane__new-word">, unsaved</);
    expect(t).toContain(`title="${PATH1} · Opened by Codex · Unsaved edits"`);
  });
});

describe("SidePane: preview tabs", () => {
  const tab1 = (out: string) => out.match(/<button[^>]*id="pane-tab-t1"[\s\S]*?<\/button>/)![0];
  it("a preview tab's name is italic and read with \", preview\"", () => {
    const t = tab1(html({ tabs: [{ ...tabs[0]!, preview: true }, tabs[1]!] }));
    expect(t).toContain("side-pane__tab--preview");
    expect(t).toMatch(/side-pane__new-word[^>]*>, preview</);
    const plain = tab1(html());
    expect(plain).not.toContain("side-pane__tab--preview");
    expect(plain).not.toContain(", preview");
  });
  it("a preview tab's tooltip says how to keep it", () => {
    expect(tabTitle({ path: "src/a.ts", openedBy: null, preview: true })).toBe("src/a.ts · Preview: double-click to keep");
    expect(tab1(html({ tabs: [{ ...tabs[0]!, preview: true }, tabs[1]!] }))).toContain(`title="${PATH1} · Preview: double-click to keep"`);
  });
});
