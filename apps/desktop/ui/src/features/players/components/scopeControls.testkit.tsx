// Test-only: renders the three header controls together, the way 005's shell mounts them.
import type { ProfileEntryDto } from "@/ipc/bindings";
import { mockCommands } from "@/ipc/mocks";
import { renderWithRouter } from "@/shared/testing/renderWithRouter";
import { ALL_PLAYERS, OTHER_PROFILE, SELF_PROFILE } from "../fixtures";
import { MergeCompareToggle } from "./MergeCompareToggle";
import { NotSelfBanner } from "./NotSelfBanner";
import { ScopePicker } from "./ScopePicker";

export function renderControls(path: string, profiles: ProfileEntryDto[] = [SELF_PROFILE, OTHER_PROFILE, ALL_PLAYERS]) {
  const calls = mockCommands({ playersListProfiles: () => profiles });
  const view = renderWithRouter(
    <>
      <ScopePicker />
      <MergeCompareToggle />
      <NotSelfBanner />
    </>,
    { path },
  );
  return { calls, ...view };
}
