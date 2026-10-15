/**
 * Session numbers. Every open terminal has one, shown wherever the session is named, so "go talk to session 6" means one
 * terminal. A session keeps its number until it closes; a new session takes the lowest number nobody holds. Numbers are
 * 1-based, so the first nine are reachable with Ctrl+1 to Ctrl+9.
 */
export function nextSessionNumber(taken: readonly number[]): number {
  const held = new Set(taken.filter((n) => Number.isInteger(n) && n >= 1));
  let n = 1;
  while (held.has(n)) n++;
  return n;
}

/** "Session 6", the words a person (or an agent) says and a screen reader reads. */
export const sessionWord = (n: number) => `Session ${n}`;

/** The line Marshell shows where a number appears for the first time, and in the badge's tooltip. */
export const SESSION_NUMBER_HINT = "Say “session 6” to an agent and Marshell knows which terminal you mean";
