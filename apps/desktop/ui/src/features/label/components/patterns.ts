import type { PatternDefDto } from "@/ipc/bindings";

export interface AxisGroup {
  axis: string;
  patterns: PatternDefDto[];
}

/** Taxonomy order is meaningful (the CLI legend follows it), so groups keep first-seen order. */
export function groupByAxis(taxonomy: readonly PatternDefDto[]): AxisGroup[] {
  const groups: AxisGroup[] = [];
  for (const pattern of taxonomy) {
    const group = groups.find((g) => g.axis === pattern.axis);
    if (group === undefined) {
      groups.push({ axis: pattern.axis, patterns: [pattern] });
    } else {
      group.patterns.push(pattern);
    }
  }
  return groups;
}

/** `7k.regular.jack` → `regular_jack`: the keymode prefix does not change the axis name. */
export function axisKey(axis: string): string {
  return axis.split(".").slice(1).join("_");
}

export function patternName(id: string): string {
  return (id.split(".").at(-1) ?? id).replaceAll("_", " ");
}

/** `needle` is already trimmed and lower-cased; an empty one matches everything. */
export function matchesSearch(pattern: PatternDefDto, needle: string): boolean {
  return [patternName(pattern.id), pattern.key, pattern.description].some((field) =>
    field.toLowerCase().includes(needle),
  );
}

export interface FamilyGroup {
  /** `regular` (RICE) or `ln` for 7K; any other family keeps its own section. */
  family: string;
  axes: AxisGroup[];
}

/** `7k.ln.release` → `ln`: RICE and LN are master sections whatever the keymode. */
export function axisFamily(axis: string): string {
  return axis.split(".")[1] ?? axis;
}

export function groupByFamily(groups: readonly AxisGroup[]): FamilyGroup[] {
  const families: FamilyGroup[] = [];
  for (const group of groups) {
    const family = axisFamily(group.axis);
    const existing = families.find((f) => f.family === family);
    if (existing === undefined) {
      families.push({ family, axes: [group] });
    } else {
      existing.axes.push(group);
    }
  }
  return families;
}
