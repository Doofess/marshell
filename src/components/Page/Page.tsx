import { useId, type ReactNode } from "react";
import "./Page.css";

/** A page-like screen (Doctor, Settings, the diff sheet): a heading row and a body that scrolls with a visible scrollbar. Not the live window. */
export function Page({ title, subtitle, actions, level = 2, children }: { title: string; subtitle?: string; actions?: ReactNode; level?: 1 | 2; children: ReactNode }) {
  const id = useId();
  const H = level === 1 ? "h1" : "h2";
  return (
    <section className="page" aria-labelledby={id}>
      <header className="page__head">
        <div className="page__titles">
          <H id={id} className="page__title">
            {title}
          </H>
          {subtitle && <p className="page__subtitle">{subtitle}</p>}
        </div>
        {actions && <div className="page__actions">{actions}</div>}
      </header>
      <div className="page__body">{children}</div>
    </section>
  );
}
