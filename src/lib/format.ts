/** A byte count as Windows Explorer labels it: 1024-based KB (whole), then MB and GB with one decimal. Nothing, a bad number and under 1 KB read "0 KB" and "1 KB". */
export function formatBytes(n: number): string {
  if (!(n > 0)) return "0 KB";
  const kb = n / 1024;
  if (Math.round(kb) < 1024) return `${Math.max(1, Math.round(kb))} KB`;
  const mb = kb / 1024;
  if (Number(mb.toFixed(1)) < 1024) return `${mb.toFixed(1)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}
