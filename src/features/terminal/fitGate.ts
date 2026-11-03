/**
 * Whether a terminal may fit its grid to its host now (issue #65). A hidden terminal's host measures near empty, and a fit
 * there would resize its program to a few columns, so it waits until the tab is shown, which refits.
 */
export function shouldFit(visible: boolean, width: number, height: number): boolean {
  return visible && width > 0 && height > 0;
}

/**
 * Whether a terminal sends its grid size to the core now, when no resize has done it yet. A terminal made behind another tab
 * sends nothing until it is shown, so its program starts at its real size instead of xterm's default 80x24.
 */
export function sendsFirstSize(visible: boolean, sent: boolean): boolean {
  return visible && !sent;
}
