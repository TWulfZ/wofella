import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AXIS_ICON_IDS, AxisIcon } from "./axisIcons";

// The 7K axes the taxonomy ships (ADR 0017); the app renders one axis card per id.
const TAXONOMY_AXES = [
  "7k.regular.jack",
  "7k.regular.tech",
  "7k.regular.speed",
  "7k.regular.stream",
  "7k.ln.general",
  "7k.ln.tech",
  "7k.ln.inverse",
  "7k.ln.release",
];

function svgOf(axis: string, className?: string): SVGSVGElement {
  const { container } = render(
    className === undefined ? <AxisIcon axis={axis} /> : <AxisIcon axis={axis} className={className} />,
  );
  const svg = container.querySelector("svg");
  if (svg === null) throw new Error(`no svg for ${axis}`);
  return svg;
}

function num(el: Element, attr: string): number {
  return Number(el.getAttribute(attr) ?? "0");
}

describe("AxisIcon", () => {
  it("covers every taxonomy axis", () => {
    expect([...AXIS_ICON_IDS].sort()).toEqual([...TAXONOMY_AXES].sort());
  });

  it("draws a distinct glyph for every axis and the fallback", () => {
    const markups = [...TAXONOMY_AXES, "4k.unknown.axis"].map((axis) => svgOf(axis).innerHTML);
    expect(new Set(markups).size).toBe(markups.length);
  });

  it("tags each svg with its axis, and an unknown axis with the fallback glyph", () => {
    for (const axis of TAXONOMY_AXES) {
      expect(svgOf(axis)).toHaveAttribute("data-axis-icon", axis);
    }
    expect(svgOf("7k.regular.nope")).toHaveAttribute("data-axis-icon", "fallback");
  });

  it("is decorative: hidden from assistive tech and not focusable", () => {
    for (const axis of [...TAXONOMY_AXES, "x"]) {
      const svg = svgOf(axis);
      expect(svg).toHaveAttribute("aria-hidden", "true");
      expect(svg).toHaveAttribute("focusable", "false");
    }
  });

  it("merges a caller className so cards can size and tint it", () => {
    const svg = svgOf("7k.regular.jack", "size-7 text-osu-pink");
    expect(svg.getAttribute("class")).toContain("size-7");
    expect(svg.getAttribute("class")).toContain("text-osu-pink");
  });

  it("paints only with currentColor or the accent token, and every glyph uses the accent", () => {
    for (const axis of [...TAXONOMY_AXES, "x"]) {
      const shapes = [...svgOf(axis).querySelectorAll("rect")];
      const fills = shapes.map((shape) => shape.getAttribute("fill"));
      for (const fill of fills) {
        expect(["currentColor", "var(--axis-icon-accent, currentColor)", "none"]).toContain(fill);
      }
      if (axis !== "x") expect(fills).toContain("var(--axis-icon-accent, currentColor)");
    }
  });

  // Static gallery: jsdom does not lay out, so the "story" asserts the geometry that makes the glyphs read at
  // 20–28 px — all shapes inside the 24-unit box, on whole columns, sharing one corner radius per shape kind.
  it("gallery: every glyph stays inside its viewBox with consistent note geometry", () => {
    const { container } = render(
      <div>
        {[20, 24, 28].map((px) =>
          [...TAXONOMY_AXES, "fallback.demo"].map((axis) => (
            <AxisIcon key={`${String(px)}-${axis}`} axis={axis} className={`size-[${String(px)}px]`} />
          )),
        )}
      </div>,
    );
    const svgs = [...container.querySelectorAll("svg")];
    expect(svgs).toHaveLength(3 * (TAXONOMY_AXES.length + 1));
    const noteRadii = new Set<string>();
    for (const svg of svgs) {
      expect(svg).toHaveAttribute("viewBox", "0 0 24 24");
      const rects = [...svg.querySelectorAll("rect")];
      expect(rects.length).toBeGreaterThan(0);
      for (const rect of rects) {
        const x = num(rect, "x");
        const y = num(rect, "y");
        expect(x).toBeGreaterThanOrEqual(0);
        expect(y).toBeGreaterThanOrEqual(0);
        expect(x + num(rect, "width")).toBeLessThanOrEqual(24);
        expect(y + num(rect, "height")).toBeLessThanOrEqual(24);
        if (rect.getAttribute("data-shape") === "note") noteRadii.add(rect.getAttribute("rx") ?? "");
      }
    }
    expect(noteRadii.size).toBe(1);
  });

  it("draws long notes with a body on the LN axes only", () => {
    for (const axis of TAXONOMY_AXES) {
      const bodies = svgOf(axis).querySelectorAll('[data-shape="ln-body"]').length;
      if (axis.startsWith("7k.ln.")) expect(bodies).toBeGreaterThan(0);
      else expect(bodies).toBe(0);
    }
  });
});
