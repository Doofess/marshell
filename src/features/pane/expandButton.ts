import { PANEL_LABEL, type PanelId } from "./panels";

/**
 * A panel's ⤢ button: its name, whether it is pressed, its tooltip and whether it can be used. `on` is whether this panel is the
 * session's expanded one, `others` whether another panel is shown, and `beside` whether a pane that isn't expanded would sit beside
 * the terminal here. With no other panel and no terminal beside, expanding would change nothing, so the button says so.
 */
export function expandButton(panel: PanelId, s: { on: boolean; others: boolean; beside: boolean }): { label: string; pressed: boolean; title: string; disabled: boolean } {
  const label = `Expand ${PANEL_LABEL[panel]}`;
  if (!s.others && !s.beside) return { label, pressed: false, title: "No other panel is shown", disabled: true };
  if (!s.on) return { label, pressed: false, title: label, disabled: false };
  const title = !s.others ? "Show the terminal" : s.beside ? "Show the terminal and the other panels" : "Show the other panels";
  return { label, pressed: true, title, disabled: false };
}
