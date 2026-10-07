import { openPath as pluginOpenPath, openUrl as pluginOpenUrl } from "@tauri-apps/plugin-opener";

// The capability scopes open-path to the wolluf data dir; paths outside it are rejected by Tauri, not here.
export async function openPath(path: string): Promise<void> {
  await pluginOpenPath(path);
}

// The capability scopes open-url to https://osu.ppy.sh/*; any other URL is rejected by Tauri, not here.
export async function openUrl(url: string): Promise<void> {
  await pluginOpenUrl(url);
}
