import { open } from "@tauri-apps/plugin-dialog";

/** Native folder picker; resolves to `null` when the user cancels. */
export async function pickFolder(defaultPath?: string): Promise<string | null> {
  return open({ directory: true, multiple: false, ...(defaultPath === undefined ? {} : { defaultPath }) });
}
