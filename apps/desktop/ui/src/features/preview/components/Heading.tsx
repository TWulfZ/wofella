import type { ReactNode } from "react";
import { cn } from "@/shared/lib/utils";

export type HeadingLevel = 2 | 3;

/** Section headings sit one level below the scope name when several scopes share the page. */
export function Heading({ level, id, className, children }: { level: HeadingLevel; id?: string; className?: string; children: ReactNode }) {
  const Tag = level === 2 ? "h2" : "h3";
  return (
    <Tag id={id} className={cn("font-display text-base font-semibold", className)}>
      {children}
    </Tag>
  );
}
