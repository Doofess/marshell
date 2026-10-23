export function focusPendingCard(root: Pick<ParentNode, "querySelector">, where: "drawer" | "lane"): boolean {
  const card = root.querySelector<HTMLElement>(`.${where} .approve-card[data-phase="pending"]`);
  if (!card) return false;
  card.focus();
  return true;
}
