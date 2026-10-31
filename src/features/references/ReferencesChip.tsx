import { useId } from "react";
import type { ReferenceNote } from "../../generated/ReferenceNote";
import "./ReferencesChip.css";

/**
 * The note a prompt got from the project's aliases, as a chip: "1 reference" or "<n> references". The note is its tooltip and its
 * description. It takes the focus (tabIndex 0) so a keyboard reads the description; nothing announces it by itself.
 */
export function ReferencesChip({ references }: { references: ReferenceNote | null | undefined }) {
  const id = useId();
  if (!references) return null;
  const n = references.names.length;
  return (
    <>
      <span className="references-chip" tabIndex={0} title={references.note} aria-describedby={id}>
        {n === 1 ? "1 reference" : `${n} references`}
      </span>
      <span id={id} hidden>
        {references.note}
      </span>
    </>
  );
}
