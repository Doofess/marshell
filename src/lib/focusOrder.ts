/**
 * The elements of a static render that a dialog's focus trap would walk (the FOCUSABLE query in `dialogFocus.ts`), in
 * document order, each named by its aria-label, its text (a button), or its class. Tests check a tab order with it, since
 * the suite has no DOM to query.
 */
export function focusOrder(html: string): string[] {
  const out: string[] = [];
  for (const m of html.matchAll(/<([a-z][a-z0-9]*)\b([^>]*)>/g)) {
    const tag = m[1]!;
    const attrs = m[2]!;
    const tabindex = /\btabindex="(-?\d+)"/.exec(attrs)?.[1];
    const control = ["button", "input", "select", "textarea"].includes(tag) && !/\bdisabled\b/.test(attrs);
    const focusable = (tag === "a" && /\bhref=/.test(attrs)) || control || (tabindex !== undefined && tabindex !== "-1");
    if (!focusable) continue;
    const label = /\baria-label="([^"]*)"/.exec(attrs)?.[1];
    const from = m.index + m[0].length;
    const text = tag === "button" ? html.slice(from, html.indexOf(`</${tag}>`, from)).replace(/<[^>]+>/g, "").trim() : "";
    out.push(label ?? (text || (/\bclass="([^"]*)"/.exec(attrs)?.[1] ?? tag)));
  }
  return out;
}
