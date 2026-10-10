import type { ReactNode } from "react";
import "./Chip.css";

/** A small label pill: mode, port, state. */
export function Chip({ tone = "neutral", children, title }: { tone?: "neutral" | "caution" | "accent"; children: ReactNode; title?: string }) {
  return (
    <span className="chip" data-tone={tone} title={title}>
      {children}
    </span>
  );
}
