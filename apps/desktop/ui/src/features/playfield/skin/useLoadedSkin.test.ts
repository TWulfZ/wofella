import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { SkinDto } from "@/ipc/bindings";
import { BROKEN_MIME, fakeCreateImageBitmap, file, skinDto } from "./testSkinDto";
import { useLoadedSkin } from "./useLoadedSkin";

let fake: ReturnType<typeof fakeCreateImageBitmap>;

beforeEach(() => {
  fake = fakeCreateImageBitmap();
  globalThis.createImageBitmap = fake.fn as unknown as typeof createImageBitmap;
});

afterEach(() => {
  Reflect.deleteProperty(globalThis, "createImageBitmap");
});

const SKIN_A = skinDto({ folder: "A", files: [file(10, 10)], images: [{ slot: "note.0", file: 0 }] });
const START_A: { dto: SkinDto | null } = { dto: SKIN_A };
const SKIN_B = skinDto({ folder: "B", files: [file(10, 10)], images: [{ slot: "note.1", file: 0 }] });

describe("useLoadedSkin", () => {
  it("has no skin without a DTO", () => {
    const { result } = renderHook(() => useLoadedSkin(null));
    expect(result.current).toEqual({ skin: null, failedSlots: [], loading: false });
  });

  it("decodes the DTO once and keeps it across renders", async () => {
    const { result, rerender } = renderHook(({ dto }: { dto: SkinDto | null }) => useLoadedSkin(dto), {
      initialProps: START_A,
    });
    expect(result.current.loading).toBe(true);
    await waitFor(() => {
      expect(result.current.skin?.images.has("note.0")).toBe(true);
    });
    rerender({ dto: SKIN_A });
    expect(fake.fn).toHaveBeenCalledTimes(1);
    expect(result.current.loading).toBe(false);
  });

  it("closes the old skin's bitmaps when the skin changes, and the new one's on unmount", async () => {
    const { result, rerender, unmount } = renderHook(({ dto }: { dto: SkinDto | null }) => useLoadedSkin(dto), {
      initialProps: START_A,
    });
    await waitFor(() => {
      expect(result.current.skin).not.toBeNull();
    });
    const [a] = fake.bitmaps;
    rerender({ dto: SKIN_B });
    expect(a?.close).toHaveBeenCalledTimes(1);
    expect(result.current.skin).toBeNull();
    await waitFor(() => {
      expect(result.current.skin?.images.has("note.1")).toBe(true);
    });
    const [, b] = fake.bitmaps;
    expect(b?.close).not.toHaveBeenCalled();
    unmount();
    expect(b?.close).toHaveBeenCalledTimes(1);
  });

  it("closes a skin that finishes decoding after it was replaced, without showing it", async () => {
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const inner = fake.fn.getMockImplementation();
    fake.fn.mockImplementationOnce(async (...args: Parameters<NonNullable<typeof inner>>) => {
      await gate;
      return inner?.(...args) ?? Promise.reject(new Error("no fake"));
    });
    const { result, rerender } = renderHook(({ dto }: { dto: SkinDto | null }) => useLoadedSkin(dto), {
      initialProps: START_A,
    });
    rerender({ dto: null });
    release();
    await waitFor(() => {
      expect(fake.bitmaps).toHaveLength(1);
    });
    await waitFor(() => {
      expect(fake.bitmaps[0]?.close).toHaveBeenCalledTimes(1);
    });
    expect(result.current.skin).toBeNull();
  });

  it("reports the slots whose file failed to decode", async () => {
    const dto = skinDto({
      files: [file(10, 10, { mime: BROKEN_MIME })],
      images: [{ slot: "key.2", file: 0 }],
    });
    const { result } = renderHook(() => useLoadedSkin(dto));
    await waitFor(() => {
      expect(result.current.skin).not.toBeNull();
    });
    expect(result.current.failedSlots).toEqual(["key.2"]);
    expect(result.current.skin?.images.size).toBe(0);
  });
});
