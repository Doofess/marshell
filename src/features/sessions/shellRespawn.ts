/** A shell that ends this many times within this many milliseconds is not started again (a broken shell would loop). */
export const QUICK_EXITS = { count: 3, withinMs: 10_000 } as const;

/** After the panel's shell ended at `now`: whether to start another, and the exits to keep (those within the window, this one included). */
export function shellRespawn(exits: readonly number[], now: number): { again: boolean; exits: number[] } {
  const kept = [...exits.filter((t) => now - t < QUICK_EXITS.withinMs), now];
  return { again: kept.length < QUICK_EXITS.count, exits: kept };
}
