import { createContext, useContext } from "react";
import type { OpenTarget } from "./paneState";

/** What opens a file in the side pane: a path, and optionally the line and column to show and the view to start in. */
export type OpenInPane = (target: OpenTarget) => void;

const nothing: OpenInPane = () => {};

/** The live window provides this; anything under it (a terminal's clickable paths) opens a file with `useOpenInPane()`. */
export const PaneContext = createContext<OpenInPane>(nothing);

export const useOpenInPane = (): OpenInPane => useContext(PaneContext);
