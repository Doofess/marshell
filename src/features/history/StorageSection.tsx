import { type Ref, useEffect, useRef, useState } from "react";
import type { RetentionLimits } from "../../generated/RetentionLimits";
import type { StorageReport } from "../../generated/StorageReport";
import { getStorage, previewRetention } from "../../lib/api/storage";
import { pickFolder } from "../../lib/folderPicker";
import { formatBytes } from "../../lib/format";
import type { SettingsSync } from "../appearance/settingsSync";
import { relativeTime } from "./historyState";
import { type PendingLimits, chooseBackupFolder, proposeLimits, reportAfterCleanup, storageErrorText } from "./storageActions";
import "../settings/Settings.css";
import "../notifications/NotificationsDialog.css";
import "./StorageSection.css";

const DASH = "–";
const SIZE_CHOICES: [number | null, string][] = [[null, "No size limit"], [1, "1 GB"], [2, "2 GB"], [5, "5 GB"], [10, "10 GB"], [20, "20 GB"], [50, "50 GB"]];
const AGE_CHOICES: [number | null, string][] = [[null, "Forever"], [30, "30 days"], [90, "90 days"], [180, "180 days"], [365, "1 year"]];

export const LIMITS_HINT = "Transcripts are always kept. Recordings and images of pinned and bookmarked conversations are never removed.";
export const BACKUP_HINT = 'Marshell keeps an exact copy of its history in a "Marshell history" folder there. A recording or conversation removed here is removed there too.';
/** The core's reasons for refusing a backup folder that waiting won't fix. One of them in the status is shown as it is; any other error, a folder gone missing (an unplugged drive) included, reads as Can't reach. */
const REFUSALS = [
  "Choose a folder outside Marshell's own folder.",
  "That folder holds another Marshell's backup. Choose another folder.",
  'That folder already has a "Marshell history" folder that Marshell didn\'t make. Choose another folder.',
];

const DATE = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric" });

function removalQuestion({ preview }: PendingLimits): string {
  const size = formatBytes(preview.bytes);
  const from = preview.oldest_ms === null ? null : DATE.format(new Date(preview.oldest_ms));
  if (preview.files === 1) return `This removes 1 recording or image now (${size})${from ? `, from ${from}` : ""}.`;
  return `This removes ${preview.files} recordings and images now (${size})${from ? `, the oldest from ${from}` : ""}.`;
}

function cleanupLine(run: NonNullable<StorageReport["last_cleanup"]>, now: number): string {
  const when = `Last cleanup ${relativeTime(now - run.at_ms)}`;
  if (run.files === 0) return `${when}: nothing to remove.`;
  return `${when}: removed ${run.files} ${run.files === 1 ? "file" : "files"} (${formatBytes(run.bytes)}).`;
}

function backupLine(b: StorageReport["backup"], now: number): string {
  if (b.error) return REFUSALS.includes(b.error) ? b.error : `Can't reach ${b.dir}. Marshell will try again in a minute.`;
  return b.last_copy_ms === null ? "Not copied yet." : `Last copy ${relativeTime(now - b.last_copy_ms)}.`;
}

export type StorageSectionViewProps = {
  /** Null until the report has arrived: sizes show an en dash and no control shows. */
  report: StorageReport | null;
  /** A change that would remove files, waiting for a yes. */
  pending: PendingLimits | null;
  error: string | null;
  busy: boolean;
  now: number;
  sizeRef?: Ref<HTMLSelectElement>;
  ageRef?: Ref<HTMLSelectElement>;
  onLimits: (next: RetentionLimits, field: "size" | "age") => void;
  onConfirm: () => void;
  onCancel: () => void;
  onChoose: () => void;
  onStop: () => void;
};

/** Settings → History → Storage: what Marshell keeps on disk, the limits on recordings and images, and the backup folder. */
export function StorageSectionView({ report, pending, error, busy, now, sizeRef, ageRef, onLimits, onConfirm, onCancel, onChoose, onStop }: StorageSectionViewProps) {
  const bytes = (n: number | undefined) => (n === undefined ? DASH : formatBytes(n));
  const rows: [string, number | undefined][] = [
    ["Transcripts", report?.transcripts],
    ["Recordings", report?.recordings],
    ["Images", report?.attachments],
    ["Snapshots", report?.snapshots],
    ["Logs", report?.logs],
    ["Search index", report?.index],
    ["Total", report ? report.transcripts + report.recordings + report.attachments + report.snapshots + report.logs + report.index : undefined],
  ];
  const shown = pending?.limits ?? report?.limits;
  const limited = report !== null && (report.limits.limit_gb !== null || report.limits.limit_days !== null);
  return (
    <section className="notifications-dialog__group storage" aria-labelledby="storage-title">
      <h3 id="storage-title" className="notifications-dialog__name">
        Storage
      </h3>
      <dl className="storage__rows">
        {rows.map(([label, n]) => (
          <div key={label} className="storage__row">
            <dt>{label}</dt>
            <dd>{bytes(n)}</dd>
          </div>
        ))}
      </dl>
      {report && shown && (
        <>
          <div className="storage__field">
            <label className="field__label" htmlFor="storage-size">
              Keep recordings and images up to
            </label>
            <select id="storage-size" ref={sizeRef} className="field__control" aria-describedby="storage-limits-hint" value={shown.limit_gb ?? ""} onChange={(e) => onLimits({ ...shown, limit_gb: e.target.value === "" ? null : Number(e.target.value) }, "size")}>
              {SIZE_CHOICES.map(([v, text]) => (
                <option key={text} value={v ?? ""}>
                  {text}
                </option>
              ))}
            </select>
          </div>
          <div className="storage__field">
            <label className="field__label" htmlFor="storage-age">
              Keep recordings and images for
            </label>
            <select id="storage-age" ref={ageRef} className="field__control" aria-describedby="storage-limits-hint" value={shown.limit_days ?? ""} onChange={(e) => onLimits({ ...shown, limit_days: e.target.value === "" ? null : Number(e.target.value) }, "age")}>
              {AGE_CHOICES.map(([v, text]) => (
                <option key={text} value={v ?? ""}>
                  {text}
                </option>
              ))}
            </select>
          </div>
          <p id="storage-limits-hint" className="field__help">
            {LIMITS_HINT}
          </p>
          {pending && (
            <div className="storage__confirm">
              <p className="notifications-dialog__line storage__ask">{removalQuestion(pending)}</p>
              <div className="storage__actions">
                <button type="button" className="notifications-dialog__btn" disabled={busy} onClick={onConfirm}>
                  Remove and save
                </button>
                <button type="button" className="notifications-dialog__btn" disabled={busy} onClick={onCancel}>
                  Cancel
                </button>
              </div>
            </div>
          )}
          {limited && report.last_cleanup && <p className="notifications-dialog__line">{cleanupLine(report.last_cleanup, now)}</p>}
          <div className="storage__field">
            <span className="field__label" id="storage-backup-label">
              Backup folder
            </span>
            <span className="storage__path" aria-labelledby="storage-backup-label">
              {report.backup.dir ?? "None"}
            </span>
            <div className="storage__actions">
              <button type="button" className="notifications-dialog__btn" disabled={busy} onClick={onChoose}>
                Choose folder…
              </button>
              {report.backup.dir && (
                <button type="button" className="notifications-dialog__btn" disabled={busy} onClick={onStop}>
                  Stop backup
                </button>
              )}
            </div>
          </div>
          <p className="field__help">{BACKUP_HINT}</p>
          {report.backup.dir && <p className="notifications-dialog__line">{backupLine(report.backup, now)}</p>}
        </>
      )}
      {error && <p className="notifications-dialog__error">{error}</p>}
    </section>
  );
}

/** The Storage section on the app's settings. Limits and the backup folder are saved through `SettingsSync`; the sizes and the report are read again after each save. */
export function StorageSection({ sync }: { sync?: SettingsSync }) {
  const [report, setReport] = useState<StorageReport | null>(null);
  const [pending, setPending] = useState<PendingLimits | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [refocus, setRefocus] = useState(0);
  const field = useRef<"size" | "age">("size");
  const sizeRef = useRef<HTMLSelectElement>(null);
  const ageRef = useRef<HTMLSelectElement>(null);
  const live = useRef(true);

  const reload = async () => {
    const next = await getStorage();
    if (live.current) setReport(next);
  };
  useEffect(() => {
    live.current = true;
    reload().catch((e) => live.current && setError(storageErrorText(e)));
    return () => {
      live.current = false;
    };
  }, []);
  // Focus stays on the select the person was using, also after the confirm line has gone.
  useEffect(() => {
    if (refocus > 0) (field.current === "size" ? sizeRef : ageRef).current?.focus();
  }, [refocus]);

  const run = async (work: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await work();
    } catch (e) {
      if (live.current) setError(storageErrorText(e));
    } finally {
      if (live.current) setBusy(false);
    }
  };
  const saveLimits = async (limits: RetentionLimits) => {
    if (!sync) return;
    const before = report?.last_cleanup?.at_ms ?? null;
    await sync.setHistoryLimits(limits);
    setPending(null);
    // A limit runs a cleanup on the core's storage thread just after the save; wait for it so the sizes and the last-cleanup line show what it removed.
    const limited = limits.limit_gb !== null || limits.limit_days !== null;
    const next = limited ? await reportAfterCleanup(getStorage, before) : await getStorage();
    if (live.current) setReport(next);
  };

  return (
    <StorageSectionView
      report={report}
      pending={pending}
      error={error}
      busy={busy}
      now={Date.now()}
      sizeRef={sizeRef}
      ageRef={ageRef}
      onLimits={(limits, which) => {
        field.current = which;
        void run(async () => {
          if (!sync) return;
          const ask = await proposeLimits(previewRetention, saveLimits, limits);
          setPending(ask);
        });
      }}
      onConfirm={() =>
        void run(async () => {
          if (pending) await saveLimits(pending.limits);
          setRefocus((n) => n + 1);
        })
      }
      onCancel={() => {
        setPending(null);
        setRefocus((n) => n + 1);
      }}
      onChoose={() =>
        void run(async () => {
          if (!sync) return;
          if (await chooseBackupFolder(pickFolder, (dir) => sync.setHistoryBackupDir(dir), report?.backup.dir ?? null)) await reload();
        })
      }
      onStop={() =>
        void run(async () => {
          if (!sync) return;
          await sync.setHistoryBackupDir(null);
          await reload();
        })
      }
    />
  );
}
