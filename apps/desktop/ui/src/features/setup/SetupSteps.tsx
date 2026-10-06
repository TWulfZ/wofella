import { useTranslation } from "react-i18next";
import { cn } from "cn";

const STEPS = ["install", "identity"] as const;

export type SetupStep = (typeof STEPS)[number];

export function SetupSteps({ current }: { current: SetupStep }) {
  const { t } = useTranslation();
  const currentIndex = STEPS.indexOf(current);
  return (
    <ol aria-label={t("setup.steps.label")} className="flex items-center gap-3 text-sm">
      {STEPS.map((step, i) => {
        const isCurrent = i === currentIndex;
        const isDone = i < currentIndex;
        return (
          <li
            key={step}
            aria-current={isCurrent ? "step" : undefined}
            className={cn(
              "flex items-center gap-2 font-medium whitespace-nowrap",
              isCurrent ? "text-primary" : isDone ? "text-foreground" : "text-muted-foreground",
              // The connector is drawn by the later step so the text content stays "<n><label>".
              i > 0 && "before:bg-border before:mr-1 before:h-px before:w-8 before:content-['']",
            )}
          >
            <span
              className={cn(
                "tabular grid size-6 place-items-center rounded-full text-xs font-bold",
                isCurrent
                  ? "bg-primary text-primary-foreground"
                  : isDone
                    ? "bg-primary/15 text-primary"
                    : "bg-muted text-muted-foreground",
              )}
            >
              {i + 1}
            </span>
            {t(`setup.steps.${step}`)}
          </li>
        );
      })}
    </ol>
  );
}
