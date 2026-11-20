import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MainWindow } from "./MainWindow";

const html = (p: Partial<Parameters<typeof MainWindow>[0]> = {}) => renderToStaticMarkup(<MainWindow width={1180} height={800} selectedId="working" {...p} />);

describe("MainWindow: window frame", () => {
  const controls = { maximized: false, onMinimize: () => {}, onToggleMaximize: () => {}, onClose: () => {} };
  const drag = (h: string) => (h.match(/<\w+[^>]*>/g) ?? []).filter((t) => t.includes("data-tauri-drag-region"));
  it("lets the bar's empty space move the window, and never a button", () => {
    const tags = drag(html({ windowControls: controls }));
    for (const c of ["window__bar", "window__bar-side", "window__bar-main", "window__bar-end"]) expect(tags.some((t) => t.includes(`class="${c}"`)), c).toBe(true);
    expect(tags.some((t) => t.startsWith("<button"))).toBe(false);
  });
  it("keeps chips out of the drag region", () => {
    expect(html()).toMatch(/<span class="chip"[^>]*data-tauri-drag-region="false"/);
  });
  it("draws the three controls for Windows only", () => {
    expect(html({ windowControls: controls })).toContain('aria-label="Minimize"');
    expect(html({ os: "mac", windowControls: controls })).not.toContain('aria-label="Minimize"');
    expect(html({ os: "linux", windowControls: controls })).not.toContain('aria-label="Close"');
    const inside = html({ windowControls: controls }).match(/<div class="window__controls">(.*?)<\/div>/)?.[1] ?? "";
    expect(inside.match(/<button/g)).toHaveLength(3);
  });
  it("keeps two hidden probes beside the controls, out of the tab order", () => {
    const h = html({ windowControls: controls });
    const probes = h.match(/<div hidden="">(.*?)<\/div>/)?.[1] ?? "";
    expect(probes.match(/<button[^>]*>/g)).toHaveLength(2);
    expect(probes).toContain('tabindex="-1"');
    expect(probes).toContain('data-look="hover"');
    expect(probes).toContain("window__control--close");
    expect(h.indexOf('<div hidden="">')).toBeGreaterThan(h.indexOf('class="window__controls"'));
    expect(html({ os: "mac", windowControls: controls })).not.toContain("data-look");
  });
  it("names the middle button Restore while maximized", () => {
    const h = html({ windowControls: { ...controls, maximized: true } });
    expect(h).toContain('aria-label="Restore"');
    expect(h).not.toContain('aria-label="Maximize"');
    expect(html({ windowControls: controls })).toContain('aria-label="Maximize"');
  });
});
