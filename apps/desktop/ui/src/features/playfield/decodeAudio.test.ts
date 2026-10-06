import { describe, expect, it } from "vitest";
import { base64ToBytes, decodeBase64Audio } from "./decodeAudio";

describe("base64ToBytes", () => {
  it("decodes every byte value", () => {
    expect([...base64ToBytes("AAH/gA==")]).toEqual([0x00, 0x01, 0xff, 0x80]);
    expect(base64ToBytes("")).toHaveLength(0);
  });
});

describe("decodeBase64Audio", () => {
  it("hands the decoded bytes to the context and returns its buffer", async () => {
    const decoded = { duration: 3 };
    const seen: number[][] = [];
    const ctx = {
      async decodeAudioData(data: ArrayBuffer) {
        seen.push([...new Uint8Array(data)]);
        return Promise.resolve(decoded);
      },
    };
    await expect(decodeBase64Audio(ctx, "SUQz")).resolves.toBe(decoded);
    expect(seen).toEqual([[0x49, 0x44, 0x33]]);
  });

  it("rejects malformed base64 without calling the decoder", async () => {
    let calls = 0;
    const ctx = {
      async decodeAudioData() {
        calls++;
        return Promise.resolve({ duration: 0 });
      },
    };
    await expect(decodeBase64Audio(ctx, "not base64!")).rejects.toThrow();
    expect(calls).toBe(0);
  });
});
