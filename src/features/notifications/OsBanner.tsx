import { Logo } from "../../components/Logo/Logo";
import type { Banner, Os } from "./banner";
import "./OsBanner.css";

/**
 * A banner as the OS shows it, drawn as a neutral layout: the content and the buttons are the design, the operating
 * system's own styling is not copied. The caption (a story's wrapper, not the banner) says so, and says when a frame is for
 * a platform that is not tested yet.
 */
export function OsBanner({ os, banner, caption = false }: { os: Os; banner: Banner; caption?: boolean }) {
  return (
    <figure className="os-banner-frame">
      <article className="os-banner" data-os={os} data-raises-window="false" aria-label={`${banner.title}. ${banner.body}`}>
        <header className="os-banner__head">
          <Logo size={16} decorative />
          <p className="os-banner__app">Marshell</p>
          <p className="os-banner__meta" dir="auto">
            {banner.meta}
          </p>
        </header>
        <p className="os-banner__title" dir="auto">
          {banner.title}
        </p>
        <p className="os-banner__body" dir="auto">
          {banner.body}
        </p>
        <div className="os-banner__actions">
          {banner.actions.map((a) => (
            <button key={a} type="button" className="os-banner__btn">
              {a}
            </button>
          ))}
        </div>
      </article>
      {caption && (
        <figcaption className="os-banner__note">
          Layout and content, not the operating system&apos;s own styling.
          {os !== "windows" && <> Design only: macOS and Linux are not tested yet.</>}
        </figcaption>
      )}
    </figure>
  );
}
