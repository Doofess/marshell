import { createContext, useContext } from "react";
import type { PreviewEntry } from "./previewState";

/** What opens a file in the preview: a path, and optionally the line and column to show and the view to start in. */
export type OpenPreview = (entry: Omit<PreviewEntry, "view"> & { view?: PreviewEntry["view"] }) => void;

const nothing: OpenPreview = () => {};

/** The live window provides this; anything under it (a terminal's clickable paths, quick open) opens a file with `useOpenPreview()`. */
export const PreviewContext = createContext<OpenPreview>(nothing);

export const useOpenPreview = (): OpenPreview => useContext(PreviewContext);
