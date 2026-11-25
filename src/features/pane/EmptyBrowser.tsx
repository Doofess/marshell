import { EmptyList, EmptyPanel, FileIcon, GlobeIcon, PlusIcon, type EmptyAction } from "./EmptyPanel";

/** Why a web page can't be opened where the system has no pages yet. */
export const PAGES_UNAVAILABLE = "Not available on this system yet";

/**
 * The Browser panel with no tabs. Where pages are available it leads with "New tab" and lists the ports the session listens on;
 * where they aren't, an HTML file is the one thing it can show, so "Open HTML file…" leads and no port is offered.
 */
export function EmptyBrowser({ available, ports, onNewTab, onOpenHtml, onPort }: { available: boolean; ports: readonly { port: number; url: string }[]; onNewTab(): void; onOpenHtml(): void; onPort(url: string): void }) {
  const newTab: EmptyAction = { label: "New tab", icon: PlusIcon, disabled: available ? undefined : PAGES_UNAVAILABLE, onClick: onNewTab };
  const openHtml: EmptyAction = { label: "Open HTML file…", icon: FileIcon, onClick: onOpenHtml };
  const listed = available ? ports : [];
  const line = !available
    ? "Open an HTML file. Web pages aren't available on this system yet."
    : listed.length > 0
      ? "Open an address, an HTML file or a local server."
      : "Open an address or an HTML file.";
  return (
    <EmptyPanel icon={GlobeIcon} title="No pages open" line={line} actions={available ? [newTab, openHtml] : [openHtml, newTab]}>
      {listed.length > 0 && (
        <EmptyList heading="Local servers">
          {listed.map((p) => (
            <li key={p.port}>
              <button type="button" className="empty-panel__row" title={p.url} onClick={() => onPort(p.url)}>
                <span className="empty-panel__dot" aria-hidden="true" />
                <span className="empty-panel__name">localhost:{p.port}</span>
              </button>
            </li>
          ))}
        </EmptyList>
      )}
    </EmptyPanel>
  );
}
