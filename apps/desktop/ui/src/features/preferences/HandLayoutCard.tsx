import { useQuery } from "@tanstack/react-query";
import { Check, CircleCheck } from "lucide-react";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import { useErrorText } from "@/ipc/errorText";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/shared/ui/card";
import { LayoutDiagram } from "./LayoutDiagram";
import { useLayoutName } from "./layoutName";
import { handLayoutQuery, handLayoutsQuery, useSetHandLayout } from "./queries";

const OPTION =
  "group/option bg-surface-raised ring-border hover:bg-accent has-[:checked]:ring-primary has-[:checked]:bg-accent has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-card relative flex cursor-pointer flex-col gap-2 rounded-lg p-3 ring-1 motion-safe:transition-colors has-[:checked]:ring-2 has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-offset-2";

const SWATCH = "inline-block size-2.5 rounded-sm";

export function HandLayoutCard({ keymode }: { keymode: number }) {
  const { t } = useTranslation();
  const errorText = useErrorText();
  const layoutName = useLayoutName();
  const titleId = useId();
  const layouts = useQuery(handLayoutsQuery(keymode));
  const current = useQuery(handLayoutQuery(keymode));
  const setLayout = useSetHandLayout(keymode);
  const selected = setLayout.isPending ? setLayout.variables : current.data;
  const loadError = layouts.error ?? current.error;

  return (
    <Card>
      <CardHeader>
        <CardTitle id={titleId}>{t("settings.handLayout.title", { keymode })}</CardTitle>
        <CardDescription>{t("settings.handLayout.description")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        {loadError !== null ? (
          <p role="alert" className="text-destructive text-sm">
            {errorText(loadError)}
          </p>
        ) : layouts.data === undefined || current.data === undefined ? (
          <div aria-busy="true" className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {[0, 1, 2].map((i) => (
              <div key={i} className="bg-surface-raised h-24 rounded-lg motion-safe:animate-pulse" />
            ))}
          </div>
        ) : (
          <div role="radiogroup" aria-labelledby={titleId} className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {layouts.data.map((layout) => {
              const checked = selected === layout.id;
              return (
                <label key={layout.id} className={OPTION}>
                  <input
                    type="radio"
                    name={`hand-layout-${String(keymode)}`}
                    value={layout.id}
                    className="sr-only"
                    checked={checked}
                    onChange={() => {
                      setLayout.mutate(layout.id);
                    }}
                  />
                  <LayoutDiagram columns={layout.columns} className="h-10 w-full" />
                  <span className="flex items-center justify-between gap-2 text-sm font-medium">
                    <span className="tabular">{layoutName(layout.id)}</span>
                    {checked && <Check aria-hidden="true" className="text-primary size-4 shrink-0" />}
                  </span>
                </label>
              );
            })}
          </div>
        )}
        <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2">
          <ul className="text-muted-foreground flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
            <li className="flex items-center gap-1.5">
              <span aria-hidden="true" className={`${SWATCH} bg-osu-pink`} />
              {t("settings.handLayout.legend.left")}
            </li>
            <li className="flex items-center gap-1.5">
              <span aria-hidden="true" className={`${SWATCH} bg-osu-blue`} />
              {t("settings.handLayout.legend.right")}
            </li>
            <li className="flex items-center gap-1.5">
              <span aria-hidden="true" className={`${SWATCH} bg-osu-purple`} />
              {t("settings.handLayout.legend.both")}
            </li>
            <li className="flex items-center gap-1.5">
              <span aria-hidden="true" className="bg-background border-foreground inline-block size-2.5 rounded-full border" />
              {t("settings.handLayout.legend.thumb")}
            </li>
            <li className="flex items-center gap-1.5">
              <span aria-hidden="true" className="border-foreground inline-block h-3 border-l border-dashed" />
              {t("settings.handLayout.legend.split")}
            </li>
          </ul>
          {setLayout.isSuccess && (
            <p role="status" className="text-success flex items-center gap-1.5 text-sm">
              <CircleCheck aria-hidden="true" className="size-4" />
              {t("settings.handLayout.saved")}
            </p>
          )}
          {setLayout.isError && (
            <p role="alert" className="text-destructive text-sm">
              {errorText(setLayout.error)}
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
