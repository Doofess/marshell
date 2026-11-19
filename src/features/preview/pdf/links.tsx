import type { Dest } from "./dest";

export type NamedAction = "NextPage" | "PrevPage" | "FirstPage" | "LastPage" | "GoBack" | "GoForward";

export type LinkGoal = { kind: "dest"; dest: Dest } | { kind: "web"; url: string } | { kind: "action"; action: NamedAction } | { kind: "other" };

/** A link's box as fractions of the page. */
export type PageLink = { box: { left: number; top: number; width: number; height: number }; goal: LinkGoal; label: string };

const ACTION_LABEL: Record<NamedAction, string> = {
  NextPage: "Next page",
  PrevPage: "Previous page",
  FirstPage: "First page",
  LastPage: "Last page",
  GoBack: "Back",
  GoForward: "Forward",
};

/** What an annotation leads to: `url` is read, never `unsafeUrl`, and only http: and https: leave the file. */
export function goalOf(data: { url?: unknown; dest?: unknown; action?: unknown }): LinkGoal {
  if (typeof data.url === "string") {
    try {
      const u = new URL(data.url);
      if (u.protocol === "http:" || u.protocol === "https:") return { kind: "web", url: data.url };
    } catch {
      // Not an address: it is some other link.
    }
    return { kind: "other" };
  }
  if (typeof data.dest === "string" || Array.isArray(data.dest)) return { kind: "dest", dest: data.dest };
  if (typeof data.action === "string" && Object.hasOwn(ACTION_LABEL, data.action)) return { kind: "action", action: data.action as NamedAction };
  return { kind: "other" };
}

export function labelOf(goal: LinkGoal): string {
  switch (goal.kind) {
    case "web":
      return goal.url;
    case "dest":
      return "Link within the file";
    case "action":
      return ACTION_LABEL[goal.action];
    case "other":
      return "Link";
  }
}

/** The links among a page's annotations, with their boxes as fractions of the page. `toView` maps a PDF rectangle to the view's. */
export function linksOf(annotations: readonly unknown[], view: { width: number; height: number; toView(rect: readonly number[]): number[] }): PageLink[] {
  const out: PageLink[] = [];
  if (!(view.width > 0) || !(view.height > 0)) return out;
  for (const a of annotations) {
    const data = a as { subtype?: unknown; rect?: unknown; url?: unknown; dest?: unknown; action?: unknown } | null;
    if (!data || data.subtype !== "Link" || !Array.isArray(data.rect) || data.rect.length < 4) continue;
    const [x1, y1, x2, y2] = view.toView(data.rect as number[]);
    if (![x1, y1, x2, y2].every((n) => typeof n === "number" && Number.isFinite(n))) continue;
    const goal = goalOf(data);
    out.push({
      box: { left: Math.min(x1!, x2!) / view.width, top: Math.min(y1!, y2!) / view.height, width: Math.abs(x2! - x1!) / view.width, height: Math.abs(y2! - y1!) / view.height },
      goal,
      label: labelOf(goal),
    });
  }
  return out;
}

const pct = (n: number) => `${n * 100}%`;

/** The links over one page. Spans with a role, never anchors: a link must not be able to navigate the app's webview. */
export function PageLinks({ links, onFollow }: { links: readonly PageLink[]; onFollow(goal: LinkGoal): void }) {
  if (links.length === 0) return null;
  return (
    <div className="pdf__links">
      {links.map((l, i) => (
        <span
          key={i}
          role="link"
          tabIndex={0}
          className="pdf__link"
          aria-label={l.label}
          title={l.goal.kind === "web" ? l.label : undefined}
          // Physical, as the page is: a right-to-left pane doesn't mirror a PDF.
          style={{ left: pct(l.box.left), top: pct(l.box.top), width: pct(l.box.width), height: pct(l.box.height) }}
          onClick={() => onFollow(l.goal)}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            onFollow(l.goal);
          }}
        />
      ))}
    </div>
  );
}
