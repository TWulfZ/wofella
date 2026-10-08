import { WandSparkles } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";
import { RateCopyDialog, type RateCopyTarget, useRateText } from "./RateCopyDialog";

export interface RateCopyActionParams {
  minRateMilli: number;
  maxRateMilli: number;
  stepMilli: number;
  defaultRateMilli: number;
}

export const RATE_COPY_ACTION_PARAMS: RateCopyActionParams = {
  minRateMilli: 700,
  maxRateMilli: 1500,
  stepMilli: 50,
  defaultRateMilli: 1100,
};

// 1.00x is the chart itself: the backend always refuses it (`identity_rate`), so it is never offered.
const NOMINAL_RATE_MILLI = 1000;

function rateChoices({ minRateMilli, maxRateMilli, stepMilli }: RateCopyActionParams): number[] {
  const rates: number[] = [];
  for (let rate = minRateMilli; rate <= maxRateMilli; rate += stepMilli) {
    if (rate !== NOMINAL_RATE_MILLI) {
      rates.push(rate);
    }
  }
  return rates;
}

/** A rate picker plus "Generate rate copy" for a chart shown outside the recommendations (chart details). */
export function RateCopyAction({ md5, chartLabel }: { md5: string; chartLabel: string }) {
  const { t } = useTranslation();
  const rateText = useRateText();
  const id = useId();
  const [rateMilli, setRateMilli] = useState(RATE_COPY_ACTION_PARAMS.defaultRateMilli);
  const [target, setTarget] = useState<RateCopyTarget | null>(null);
  return (
    <div className="flex flex-wrap items-center gap-2">
      <label htmlFor={id} className="text-foreground/90 text-sm">
        {t("rateCopy.rate")}
      </label>
      <select
        id={id}
        value={String(rateMilli)}
        onChange={(e) => {
          setRateMilli(Number(e.target.value));
        }}
        className="border-input bg-background/80 h-8 rounded-md border px-2 text-sm tabular-nums"
      >
        {rateChoices(RATE_COPY_ACTION_PARAMS).map((rate) => (
          <option key={rate} value={String(rate)}>
            {rateText(rate)}
          </option>
        ))}
      </select>
      <Button
        type="button"
        size="sm"
        onClick={() => {
          setTarget({ md5, rateMilli, chartLabel });
        }}
      >
        <WandSparkles aria-hidden="true" />
        {t("rateCopy.trigger")}
      </Button>
      <RateCopyDialog
        target={target}
        onOpenChange={(open) => {
          if (!open) {
            setTarget(null);
          }
        }}
      />
    </div>
  );
}
