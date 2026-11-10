import type { Os } from "../../lib/keymap";

/** Whether this system has browser tabs: Windows only for now. Where it is false, web addresses keep opening the system browser. */
export const browserTabs = (os: Os | undefined): boolean => os === "windows";
