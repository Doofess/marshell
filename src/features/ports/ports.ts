import { openUrl } from "@tauri-apps/plugin-opener";
import type { MenuItem } from "../../components/ContextMenu/menuKeys";
import type { PortChip } from "../../generated/PortChip";

const LOOPBACK_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

/** True for an http(s) URL with no userinfo whose host is exactly localhost, 127.0.0.1 or [::1]. The core checks too; this is the check before anything opens. */
export function isLoopbackUrl(url: string): boolean {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return false;
  }
  return (u.protocol === "http:" || u.protocol === "https:") && u.username === "" && u.password === "" && LOOPBACK_HOSTS.has(u.hostname);
}

/** What a chip says: the host and the port, `localhost:5173`. */
export function chipLabel(chip: PortChip): string {
  try {
    return `${new URL(chip.url).hostname}:${chip.port}`;
  } catch {
    return `localhost:${chip.port}`;
  }
}

type OpenPage = (url: string) => boolean;
type OpenSystem = (url: string) => Promise<unknown>;

/**
 * Opens a chip's URL, and only a loopback one: as a browser tab in the side pane where there is one to open in (`openPage`),
 * otherwise in the default browser. A failure is silent, as in the other callers.
 */
export async function openChipIn(url: string, openPage: OpenPage | null, openSystem: OpenSystem = openUrl): Promise<void> {
  if (!isLoopbackUrl(url)) return;
  if (openPage) {
    openPage(url);
    return;
  }
  await openSystem(url).catch(() => {});
}

/** Opens a chip's URL in the default browser, and only a loopback one. */
export const openChip = (url: string): Promise<void> => openChipIn(url, null);

/**
 * The items of a chip's menu: one chip gives Open and Copy; the +N chip gives an Open item for each chip it holds. The id ends in the
 * chip's index. Where chips open in the side pane (`inPane`) the menu gains the way out: Open in the system browser.
 */
export function chipMenuItems(chips: readonly PortChip[], inPane = false): MenuItem[] {
  if (chips.length === 1) return [{ id: "open:0", label: "Open in browser" }, { id: "copy:0", label: "Copy URL" }, ...(inPane ? [{ id: "system:0", label: "Open in the system browser" }] : [])];
  const open = chips.map((c, i): MenuItem => ({ id: `open:${i}`, label: `Open ${chipLabel(c)}` }));
  return inPane ? [...open, ...chips.map((c, i): MenuItem => ({ id: `system:${i}`, label: `Open ${chipLabel(c)} in the system browser` }))] : open;
}

/** Runs a chip menu item; `openPage` is the side pane's opener where there is one. */
export function runChipItem(chips: readonly PortChip[], id: string, openPage: OpenPage | null = null): void {
  const [action, index] = id.split(":");
  const chip = chips[Number(index)];
  if (!chip) return;
  if (action === "open") void openChipIn(chip.url, openPage);
  else if (action === "system") void openChip(chip.url);
  else if (action === "copy") void navigator.clipboard.writeText(chip.url).catch(() => {});
}
