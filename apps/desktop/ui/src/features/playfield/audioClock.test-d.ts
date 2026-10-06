import { describe, expectTypeOf, it } from "vitest";
import type { AudioContextLike } from "./audioClock";

describe("AudioContextLike", () => {
  it("is satisfied by the real WebAudio context", () => {
    expectTypeOf<AudioContext>().toExtend<AudioContextLike>();
  });
});
