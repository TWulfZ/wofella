import type { SetupStatusDto } from "@/ipc/bindings";

export const SETUP_PATH = "/setup";
export const IDENTITY_SETUP_PATH = "/setup/identity";

function normalize(pathname: string): string {
  const trimmed = pathname.replace(/\/+$/, "");
  return trimmed === "" ? "/" : trimmed;
}

/** Where the first-run guard must send the user, or null when the requested route may render (spec 005 First run). */
export function firstRunRedirect(
  status: SetupStatusDto,
  pathname: string,
): typeof SETUP_PATH | typeof IDENTITY_SETUP_PATH | null {
  const path = normalize(pathname);
  if (status.install === null) {
    return path === SETUP_PATH ? null : SETUP_PATH;
  }
  if (!status.identityReady) {
    return path === IDENTITY_SETUP_PATH ? null : IDENTITY_SETUP_PATH;
  }
  return null;
}
