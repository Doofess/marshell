import { describe, expect, it, vi } from "vitest";
import { focusPendingCard } from "./focusPendingCard";

describe("focusPendingCard", () => {
  it.each(["drawer", "lane"] as const)("focuses the pending card in the %s", (where) => {
    const focus = vi.fn();
    const querySelector = vi.fn(() => ({ focus }));
    const root = { querySelector } as unknown as Pick<ParentNode, "querySelector">;
    expect(focusPendingCard(root, where)).toBe(true);
    expect(querySelector).toHaveBeenCalledExactlyOnceWith(`.${where} .approve-card[data-phase="pending"]`);
    expect(focus).toHaveBeenCalledOnce();
  });
  it.each(["drawer", "lane"] as const)("returns false when the %s has no pending card", (where) => {
    const querySelector = vi.fn(() => null);
    expect(focusPendingCard({ querySelector }, where)).toBe(false);
    expect(querySelector).toHaveBeenCalledExactlyOnceWith(`.${where} .approve-card[data-phase="pending"]`);
  });
});
