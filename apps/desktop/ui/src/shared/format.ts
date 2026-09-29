// Formatting only: every number shown comes from a DTO, the UI never derives domain values (§8).
export function formatDateTime(iso: string, language: string): string {
  return new Intl.DateTimeFormat(language, { dateStyle: "medium", timeStyle: "short" }).format(new Date(iso));
}

export function formatNumber(value: number, language: string): string {
  return new Intl.NumberFormat(language).format(value);
}

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;

/** Compact m:ss for job ETAs. */
export function formatEta(ms: number): string {
  const totalSeconds = Math.max(0, Math.round(ms / MS_PER_SECOND));
  const minutes = Math.floor(totalSeconds / SECONDS_PER_MINUTE);
  const seconds = totalSeconds % SECONDS_PER_MINUTE;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}
