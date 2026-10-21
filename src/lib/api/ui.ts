import type { Endpoint } from "../../generated/Endpoint";
import { api } from "./client";

/** What the window tells the core about itself, so it can tell a banner worth showing from one the person is already looking at. */
export type ViewReport = { window_focused: boolean; visible_tabs: string[] };

export const ui = {
  /** Reports whether the window has focus and which sessions are on screen. */
  putView: (ep: Endpoint, report: ViewReport) => api<void>(ep, "PUT", "/v1/ui/view", report),
};
