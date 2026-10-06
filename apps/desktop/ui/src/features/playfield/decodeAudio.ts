import type { AudioBufferLike, AudioContextLike } from "./audioClock";

/** Throws on malformed input, like `atob`. */
export function base64ToBytes(base64: string): Uint8Array<ArrayBuffer> {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export async function decodeBase64Audio(
  ctx: Pick<AudioContextLike, "decodeAudioData">,
  base64: string,
): Promise<AudioBufferLike> {
  return ctx.decodeAudioData(base64ToBytes(base64).buffer);
}
