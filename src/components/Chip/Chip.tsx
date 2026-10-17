import type { ReactNode } from "react";
import "./Chip.css";

/** A small label pill: mode, port, state. */
export function Chip({ tone = "neutral", children, title, label }: { tone?: "neutral" | "caution" | "accent"; children: ReactNode; title?: string; label?: string }) {
  return (
    <span className="chip" data-tone={tone} title={title} aria-label={label}>
      {children}
    </span>
  );
}
