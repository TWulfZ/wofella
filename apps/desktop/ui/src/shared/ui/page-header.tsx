import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export function PageHeader({
  icon: Icon,
  title,
  description,
  actions,
}: {
  icon: LucideIcon;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="bg-header/60 osu-triangles relative border-b">
      <div className="mx-auto flex w-full max-w-5xl items-center gap-4 px-6 py-7">
        <div className="bg-primary/15 text-primary grid size-11 shrink-0 place-items-center rounded-xl">
          <Icon className="size-5" aria-hidden="true" />
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <h1 className="font-display text-2xl font-bold">{title}</h1>
          {description !== undefined && <p className="text-muted-foreground text-sm">{description}</p>}
        </div>
        {actions !== undefined && <div className="ml-auto flex items-center gap-2">{actions}</div>}
      </div>
      <div aria-hidden="true" className="from-osu-pink to-osu-purple absolute inset-x-0 bottom-0 h-0.5 bg-linear-to-r" />
    </div>
  );
}
