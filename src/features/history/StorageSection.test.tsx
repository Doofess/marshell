import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { StorageReport } from "../../generated/StorageReport";
import { StorageSectionView } from "./StorageSection";

const none = () => {};
const NOW = new Date(2026, 9, 24, 12).getTime();
const MIN = 60_000;
const report = (over: Partial<StorageReport> = {}): StorageReport => ({
  transcripts: 3 * 1024 * 1024,
  recordings: 512 * 1024,
  attachments: 1.3 * 1024 ** 3,
  snapshots: 0,
  logs: 2048,
  index: 10 * 1024 * 1024,
  limits: { limit_gb: null, limit_days: null },
  last_cleanup: null,
  backup: { dir: null, last_copy_ms: null, error: null },
  ...over,
});
const view = (p: { report?: StorageReport | null; pending?: { limits: StorageReport["limits"]; preview: { files: number; bytes: number; oldest_ms: number | null } } | null; error?: string | null } = {}) =>
  renderToStaticMarkup(
    <StorageSectionView report={p.report === undefined ? report() : p.report} pending={p.pending ?? null} error={p.error ?? null} busy={false} now={NOW} onLimits={none} onConfirm={none} onCancel={none} onChoose={none} onStop={none} />,
  );

describe("StorageSectionView", () => {
  it("has the Storage heading and a row for each kind with its size, and a total", () => {
    const h = view();
    expect(h).toContain(">Storage</h3>");
    for (const [label, size] of [["Transcripts", "3.0 MB"], ["Recordings", "512 KB"], ["Images", "1.3 GB"], ["Snapshots", "0 KB"], ["Logs", "2 KB"], ["Search index", "10.0 MB"]]) {
      expect(h).toContain(`<dt>${label}</dt><dd>${size}</dd>`);
    }
    expect(h).toContain("<dt>Total</dt><dd>1.3 GB</dd>");
    expect(h).toMatch(/<dl[^>]*>/);
  });
  it("shows an en dash for every size, and no controls, before the report arrives", () => {
    const h = view({ report: null });
    expect(h.match(/<dd>–<\/dd>/g)).toHaveLength(7);
    expect(h).not.toContain("<select");
    expect(h).not.toContain("Backup folder");
  });
  it("has both selects with the exact options, set from the limits", () => {
    const h = view({ report: report({ limits: { limit_gb: 5, limit_days: 365 } }) });
    expect(h).toContain("Keep recordings and images up to");
    expect(h).toContain("Keep recordings and images for");
    for (const o of ["No size limit", "1 GB", "2 GB", "5 GB", "10 GB", "20 GB", "50 GB", "Forever", "30 days", "90 days", "180 days", "1 year"]) expect(h).toContain(`>${o}</option>`);
    expect(h).toMatch(/<option value="5" selected="">5 GB<\/option>/);
    expect(h).toMatch(/<option value="365" selected="">1 year<\/option>/);
    expect(h).toContain("Transcripts are always kept. Recordings and images of pinned and bookmarked conversations are never removed.");
  });
  it("shows the pending limits in the selects while a removal waits for a yes", () => {
    const h = view({ pending: { limits: { limit_gb: 1, limit_days: null }, preview: { files: 2, bytes: 1024, oldest_ms: NOW } } });
    expect(h).toMatch(/<option value="1" selected="">1 GB<\/option>/);
  });
  it("asks before removing 42 files, with the size and the oldest date", () => {
    const oldest = new Date(2026, 2, 3, 12).getTime();
    const h = view({ pending: { limits: { limit_gb: 1, limit_days: null }, preview: { files: 42, bytes: 1.3 * 1024 ** 3, oldest_ms: oldest } } });
    expect(h).toContain("This removes 42 recordings and images now (1.3 GB), the oldest from 3 March 2026.");
    expect(h).toContain(">Remove and save</button>");
    expect(h).toContain(">Cancel</button>");
  });
  it("words the question for one file in the singular", () => {
    const h = view({ pending: { limits: { limit_gb: null, limit_days: 30 }, preview: { files: 1, bytes: 512 * 1024, oldest_ms: new Date(2026, 0, 15, 12).getTime() } } });
    expect(h).toContain("This removes 1 recording or image now (512 KB), from 15 January 2026.");
  });
  it("asks nothing when nothing is pending", () => {
    expect(view()).not.toContain("Remove and save");
  });
  it("shows the last cleanup only while a limit is set", () => {
    const run = { at_ms: NOW - 2 * 60 * MIN, files: 3, bytes: 1.2 * 1024 * 1024 };
    expect(view({ report: report({ limits: { limit_gb: 5, limit_days: null }, last_cleanup: run }) })).toContain("Last cleanup 2 h ago: removed 3 files (1.2 MB).");
    expect(view({ report: report({ limits: { limit_gb: null, limit_days: 30 }, last_cleanup: { ...run, files: 0, bytes: 0 } }) })).toContain("Last cleanup 2 h ago: nothing to remove.");
    expect(view({ report: report({ last_cleanup: run }) })).not.toContain("Last cleanup");
    expect(view({ report: report({ limits: { limit_gb: 5, limit_days: null } }) })).not.toContain("Last cleanup");
  });
  it("shows None and no Stop button without a backup folder", () => {
    const h = view();
    expect(h).toContain("Backup folder");
    expect(h).toContain(">None<");
    expect(h).toContain(">Choose folder…</button>");
    expect(h).not.toContain("Stop backup");
    expect(h).not.toContain("Not copied yet.");
    expect(h).toContain("Marshell keeps an exact copy of its history in a &quot;Marshell history&quot; folder there. A recording or conversation removed here is removed there too.");
  });
  it("shows the folder, Stop backup and the status line for a backup", () => {
    const dir = "D:\Backups";
    const backup = (b: Partial<StorageReport["backup"]>) => view({ report: report({ backup: { dir, last_copy_ms: null, error: null, ...b } }) });
    const copied = backup({ last_copy_ms: NOW - 5 * MIN });
    expect(copied).toContain(">D:\Backups<");
    expect(copied).toContain(">Stop backup</button>");
    expect(copied).toContain("Last copy 5 min ago.");
    expect(backup({})).toContain("Not copied yet.");
    expect(backup({ error: "os error 3" })).toContain("Can&#x27;t reach D:\Backups. Marshell will try again in a minute.");
  });
  it("shows a refusal's own copy as the backup status instead of Can't reach", () => {
    const copy = "That folder holds another Marshell's backup. Choose another folder.";
    const h = view({ report: report({ backup: { dir: "D:\B", last_copy_ms: null, error: copy } }) });
    expect(h).toContain("That folder holds another Marshell&#x27;s backup. Choose another folder.");
    expect(h).not.toContain("Can&#x27;t reach");
  });
  it("shows the error text it is given", () => {
    expect(view({ error: "That folder doesn't exist." })).toContain("That folder doesn&#x27;t exist.</p>");
  });
});
