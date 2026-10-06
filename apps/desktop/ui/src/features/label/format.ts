const MS_PER_SECOND = 1000;
const MS_PER_MINUTE = 60 * MS_PER_SECOND;

/** `mm:ss.mmm`, as `wolluf label` prints window bounds. */
export function formatClock(ms: number): string {
  const sign = ms < 0 ? "-" : "";
  const abs = Math.abs(Math.trunc(ms));
  const minutes = Math.floor(abs / MS_PER_MINUTE);
  const seconds = Math.floor(abs / MS_PER_SECOND) % 60;
  const millis = abs % MS_PER_SECOND;
  return `${sign}${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}
