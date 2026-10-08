import { useTranslation } from "react-i18next";

export function usePreviewFormat() {
  const { t, i18n } = useTranslation();
  const lang = i18n.language;
  const rating = new Intl.NumberFormat(lang, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const rate = new Intl.NumberFormat(lang, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const percent = new Intl.NumberFormat(lang, { style: "percent", minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const date = new Intl.DateTimeFormat(lang, { dateStyle: "medium" });
  // `YYYY-MM` is a UTC month on the wire, so it is formatted in UTC or a western offset would show the month before.
  const month = new Intl.DateTimeFormat(lang, { month: "short", year: "numeric", timeZone: "UTC" });
  const number = new Intl.NumberFormat(lang);
  return {
    // "≈" on every rating: uncalibrated MinaCalc numbers on osu! plays (ADR 0024 Labelling).
    approx: (centi: number) => t("preview.approx", { value: rating.format(centi / 100) }),
    plain: (centi: number) => rating.format(centi / 100),
    rate: (rateMilli: number) => t("preview.topPlays.rateValue", { rate: rate.format(rateMilli / 1000) }),
    goal: (permyriad: number) => percent.format(permyriad / 10000),
    date: (ms: number) => date.format(new Date(ms)),
    month: (yyyyMm: string) => {
      const [y, m] = yyyyMm.split("-").map(Number);
      return y === undefined || m === undefined || Number.isNaN(y) || Number.isNaN(m)
        ? yyyyMm
        : month.format(new Date(Date.UTC(y, m - 1, 1)));
    },
    count: (n: number) => number.format(n),
    skillset: (id: string) => {
      const key = `preview.skillset.${id}`;
      return i18n.exists(key) ? t(key) : id;
    },
  };
}

export type PreviewFormat = ReturnType<typeof usePreviewFormat>;
