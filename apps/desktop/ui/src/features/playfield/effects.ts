// The Label preview's skin toggles: which effects the playfield draws, and which ones a loaded skin can draw.

import { DEFAULT_EFFECT_DRAW_PARAMS, type EffectDrawParams } from "./drawEffects";
import { DEFAULT_SKIN_LAYOUT_PARAMS, isStripBody, type SkinLayoutParams } from "./skinLayout";
import { type LoadedSkin, SKIN_SLOT } from "./skinModel";

export interface PlayfieldEffects {
  /** False draws percy LN bodies plainly, for reading; skins without a percy body draw the same either way. */
  percy: boolean;
  judgements: boolean;
  combo: boolean;
  keyPress: boolean;
  lighting: boolean;
}

export const DEFAULT_PLAYFIELD_EFFECTS: PlayfieldEffects = {
  percy: true,
  judgements: false,
  combo: false,
  keyPress: false,
  lighting: false,
};

/**
 * Whether `skin` has the art each toggle draws with. Judgements and combo without art fall back to text, so the UI
 * keeps them on offer either way; the procedural stage draws them as text natively and reports them supported.
 */
export function skinEffectSupport(
  skin: LoadedSkin | null,
  params: SkinLayoutParams = DEFAULT_SKIN_LAYOUT_PARAMS,
  burstChain: EffectDrawParams["maxBurstChain"] = DEFAULT_EFFECT_DRAW_PARAMS.maxBurstChain,
): Record<keyof PlayfieldEffects, boolean> {
  if (skin === null) {
    return { percy: false, judgements: true, combo: true, keyPress: false, lighting: false };
  }
  const has = (slot: Parameters<typeof skin.images.has>[0]): boolean => skin.images.has(slot);
  const columns = skin.columnWidth.map((_, i) => i);
  return {
    percy: columns.some((i) => {
      const body = skin.images.get(SKIN_SLOT.body(i));
      return body !== undefined && isStripBody(body, params);
    }),
    judgements: burstChain.some((r) => has(SKIN_SLOT.hit(r))),
    // lazer's `HasFont` looks for the 0 glyph only (`LegacySkinExtensions.cs` L135–137).
    combo: has(SKIN_SLOT.comboDigit(0)),
    keyPress: columns.some((i) => has(SKIN_SLOT.keyDown(i))),
    lighting: has(SKIN_SLOT.lightingN(0)) || has(SKIN_SLOT.lightingL(0)) || has(SKIN_SLOT.stageLight),
  };
}
