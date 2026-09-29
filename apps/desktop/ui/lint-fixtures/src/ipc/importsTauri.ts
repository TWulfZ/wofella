import { invoke } from "@tauri-apps/api/core";

export const callRaw = () => invoke("setup_status");
