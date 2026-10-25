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

/** Opens a chip's URL in the default browser, and only a loopback one. A failure is silent, as in the other callers. */
export async function openChip(url: string): Promise<void> {
  if (!isLoopbackUrl(url)) return;
  await openUrl(url).catch(() => {});
}

/** The items of a chip's menu: one chip gives Open and Copy; the +N chip gives an Open item for each chip it holds. The id ends in the chip's index. */
export function chipMenuItems(chips: readonly PortChip[]): MenuItem[] {
  if (chips.length === 1) return [{ id: "open:0", label: "Open in browser" }, { id: "copy:0", label: "Copy URL" }];
  return chips.map((c, i) => ({ id: `open:${i}`, label: `Open ${chipLabel(c)}` }));
}

/** Runs a chip menu item. */
export function runChipItem(chips: readonly PortChip[], id: string): void {
  const [action, index] = id.split(":");
  const chip = chips[Number(index)];
  if (!chip) return;
  if (action === "open") void openChip(chip.url);
  else if (action === "copy") void navigator.clipboard.writeText(chip.url).catch(() => {});
}
