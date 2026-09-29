import { openPath as pluginOpenPath } from "@tauri-apps/plugin-opener";

// The capability scopes open-path to the wolluf data dir; paths outside it are rejected by Tauri, not here.
export async function openPath(path: string): Promise<void> {
  await pluginOpenPath(path);
}
