// The app's own window, for the custom title bar. Outside Tauri (Storybook, tests, a plain browser) every call does nothing.
import type { Window } from "@tauri-apps/api/window";

export const inTauri = (): boolean => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const current = async (): Promise<Window | null> => {
  if (!inTauri()) return null;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
};

export async function minimize(): Promise<void> {
  await (await current())?.minimize();
}
export async function toggleMaximize(): Promise<void> {
  await (await current())?.toggleMaximize();
}
export async function close(): Promise<void> {
  await (await current())?.close();
}
export async function setTitle(title: string): Promise<void> {
  await (await current())?.setTitle(title);
}
export async function isMaximized(): Promise<boolean> {
  return (await (await current())?.isMaximized()) ?? false;
}

/** Calls back with the maximized state now and after every resize (maximize, restore and snapping all resize). Returns the stop function. */
export function onMaximizedChange(cb: (maximized: boolean) => void): () => void {
  if (!inTauri()) return () => {};
  let stopped = false;
  let unlisten: (() => void) | undefined;
  const report = () => void isMaximized().then((m) => !stopped && cb(m)).catch(() => {});
  void current()
    .then((w) => w?.onResized(report))
    .then((u) => {
      if (stopped) u?.();
      else unlisten = u;
    })
    .catch(() => {});
  report();
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** Marks the page as the desktop app, so the stand-in window edge the design review draws gives way to the OS's own. */
export function markDesktopHost(doc: Document = document): void {
  if (inTauri()) doc.documentElement.dataset.host = "desktop";
}
