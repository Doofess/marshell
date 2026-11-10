import { createContext, useContext } from "react";
import type { OpenTarget } from "./paneState";

/** What opens a file in the side pane: a path, and optionally the line and column to show and the view to start in. */
export type OpenInPane = (target: OpenTarget) => void;

const nothing: OpenInPane = () => {};

/** The live window provides this; anything under it (a terminal's clickable paths) opens a file with `useOpenInPane()`. */
export const PaneContext = createContext<OpenInPane>(nothing);

export const useOpenInPane = (): OpenInPane => useContext(PaneContext);

/**
 * What opens a web address as a browser tab in the side pane. `focus` is where the keyboard goes: into the page (the default), or
 * to the address box for a blank tab. False when it was refused (the opener says why).
 */
export type OpenPageInPane = (url: string, opts?: { focus: "page" | "address" }) => boolean;

/** Provided beside `PaneContext`: null where browser tabs aren't available, so a caller falls back to the system browser. */
export const PagePaneContext = createContext<OpenPageInPane | null>(null);

export const useOpenPageInPane = (): OpenPageInPane | null => useContext(PagePaneContext);
