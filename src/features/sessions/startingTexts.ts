/** What each new tab says until its first output, by tab id. */
export type StartingTexts = Record<string, string>;

/** The texts without the tab whose first output has arrived. The same object when it has no text, so nothing re-renders. */
export function without(texts: StartingTexts, id: string): StartingTexts {
  if (!(id in texts)) return texts;
  const { [id]: _gone, ...rest } = texts;
  return rest;
}

/**
 * The texts without the tabs that were listed (`seen`) and no longer are (`current`): closed before they ever printed.
 * A tab the core has not listed yet keeps its text. The same object when nothing is dropped.
 */
export function pruneClosed(texts: StartingTexts, current: ReadonlySet<string>, seen: ReadonlySet<string>): StartingTexts {
  const gone = Object.keys(texts).filter((id) => seen.has(id) && !current.has(id));
  return gone.reduce(without, texts);
}
