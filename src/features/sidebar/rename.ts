export type RenameOutcome = { kind: "commit"; name: string } | { kind: "cancel" };

/**
 * What a key in the rename box does. Enter applies the trimmed name; an empty or unchanged name is refused and the original
 * stays. Escape cancels. Every other key is just typing (null), and so is any key while an input method is composing:
 * that Enter confirms the composition, not the name.
 */
export function renameOutcome(key: string, value: string, original: string, composing = false): RenameOutcome | null {
  if (composing) return null;
  if (key === "Escape") return { kind: "cancel" };
  if (key !== "Enter") return null;
  const name = value.trim();
  return name === "" || name === original ? { kind: "cancel" } : { kind: "commit", name };
}
