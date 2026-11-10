import type { OpenPageInPane } from "../pane/PaneContext";

/**
 * What a web link in a document does. Where the side pane has browser tabs the link opens one at once ("opened", or refused and
 * said so by the opener); elsewhere it is left to ask before it goes to the system browser ("ask").
 */
export function followWebLink(url: string, openPage: OpenPageInPane | null): "opened" | "ask" {
  if (openPage === null) return "ask";
  openPage(url);
  return "opened";
}
