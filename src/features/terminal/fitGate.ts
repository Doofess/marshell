/**
 * Whether a terminal may fit its grid to its host now (issue #65). A hidden terminal's host measures near empty, and a fit
 * there would resize its program to a few columns, so it waits until the tab is shown, which refits.
 */
export function shouldFit(visible: boolean, width: number, height: number): boolean {
  return visible && width > 0 && height > 0;
}
