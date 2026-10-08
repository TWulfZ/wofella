// Test-only typed fixtures: pilot-shaped preview DTOs (ADR 0024) and the entries that resolve to them.
import type { AliasListDto, ProfileEntryDto, SkillPreviewDto } from "@/ipc/bindings";

export const SELF_4K: ProfileEntryDto = {
  ref: { kind: "profile", id: 1 },
  profileKind: "self",
  label: "Me",
  isDefault: true,
  mergeMode: "merged",
  aliasIds: [1, 2],
  scopes: [{ scopeHash: "a".repeat(64), aliasIds: [1, 2], keymode: 4 }],
};

export const ALL_PLAYERS_4K: ProfileEntryDto = {
  ref: { kind: "all_players" },
  profileKind: "all_players",
  label: "",
  isDefault: false,
  mergeMode: "merged",
  aliasIds: [1, 2, 3],
  scopes: [{ scopeHash: "c".repeat(64), aliasIds: [1, 2, 3], keymode: 4 }],
};

export const ALIASES: AliasListDto = {
  selectionVersion: 1,
  cfgUsernameAvailable: true,
  wizardNeeded: false,
  aliases: [
    { id: 1, name: "TWulfZ" },
    { id: 2, name: "TWulfZasdasdasd d jSS||" },
    { id: 3, name: "Rosalind" },
  ].map(({ id, name }) => ({
    aliasId: id,
    rawName: name,
    isEmptyName: false,
    normalizedLength: name.length,
    nPlays: 10,
    byKeymode: [{ bucket: "k4", n: 10 }],
    firstPlayedAt: null,
    lastPlayedAt: null,
    nOnline: 0,
    nOffline: 10,
    nWithReplay: 10,
    topCharts: [],
    autoMatch: null,
    decision: null,
    selected: id !== 3,
    inSelfProfile: id !== 3,
  })),
};

export function ready4k(overrides: Partial<SkillPreviewDto> = {}): SkillPreviewDto {
  return {
    scopeHash: "a".repeat(64),
    keymode: 4,
    method: "preview.etterna_rating@1",
    calcVersion: 527,
    state: "ready",
    overallCenti: 2436,
    skillsets: [
      { id: "stream", ratingCenti: 2210 },
      { id: "jumpstream", ratingCenti: 2105 },
      { id: "handstream", ratingCenti: 1990 },
      { id: "stamina", ratingCenti: 2288 },
      { id: "jackspeed", ratingCenti: 2601 },
      { id: "chordjack", ratingCenti: 2507 },
      { id: "technical", ratingCenti: 2380 },
    ],
    dan: { label: "Gamma", third: "mid", marginCenti: 81 },
    evidence: {
      counted: 312,
      tier: "ok",
      excluded: [
        { reason: "incomplete", count: 40 },
        { reason: "unsupported_mods", count: 7 },
        { reason: "pending", count: 2 },
      ],
    },
    topPlays: [
      {
        playId: "1".repeat(64),
        md5: "d".repeat(32),
        title: "Blue Zenith",
        version: "4K Insane 1.15x",
        rateMilli: 1150,
        goalPermyriad: 9650,
        overallCenti: 2712,
        dominantSkillset: "jackspeed",
        playedAtMs: Date.UTC(2026, 8, 14, 12),
      },
      {
        playId: "2".repeat(64),
        md5: "e".repeat(32),
        title: "Galaxy Collapse",
        version: "Hard",
        rateMilli: 1000,
        goalPermyriad: 9312,
        overallCenti: 2544,
        dominantSkillset: "stream",
        playedAtMs: null,
      },
    ],
    trend: [
      { month: "2026-07", overallCenti: 2210 },
      { month: "2026-08", overallCenti: 2302 },
      { month: "2026-09", overallCenti: 2436 },
    ],
    warnings: ["uncalibrated", "goal_estimated"],
    ...overrides,
  };
}

export function ready7k(overrides: Partial<SkillPreviewDto> = {}): SkillPreviewDto {
  return ready4k({
    keymode: 7,
    overallCenti: 2082,
    skillsets: [
      { id: "stream", ratingCenti: 2190 },
      { id: "jumpstream", ratingCenti: 2240 },
      { id: "handstream", ratingCenti: 2120 },
      { id: "stamina", ratingCenti: 2010 },
      { id: "jackspeed", ratingCenti: 2305 },
      { id: "chordjack", ratingCenti: 2250 },
      { id: "technical", ratingCenti: 18 },
    ],
    dan: null,
    warnings: ["uncalibrated", "goal_estimated", "k7_less_validated", "ln_not_measured", "k7_tech_not_measured"],
    ...overrides,
  });
}
