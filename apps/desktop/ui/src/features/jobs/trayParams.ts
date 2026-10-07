// Per-viewer convenience only: a blocked or cleared storage must leave the tray working on defaults.

export interface JobTrayParams {
  /** Grace period after the last job finishes, so its outcome is readable before the panel folds away. */
  autoCollapseMs: number;
}

export const JOB_TRAY_PARAMS: JobTrayParams = {
  autoCollapseMs: 4000,
};

export const JOB_TRAY_PREFS = {
  expandedKey: "wolluf.jobs.trayExpanded",
} as const;

export interface JobTrayPrefs {
  /** Null when the viewer never chose: the tray then rests collapsed. */
  read: () => boolean | null;
  write: (expanded: boolean) => void;
}

export const localJobTrayPrefs: JobTrayPrefs = {
  read: () => {
    try {
      const raw = globalThis.localStorage.getItem(JOB_TRAY_PREFS.expandedKey);
      return raw === "true" ? true : raw === "false" ? false : null;
    } catch {
      return null;
    }
  },
  write: (expanded) => {
    try {
      globalThis.localStorage.setItem(JOB_TRAY_PREFS.expandedKey, String(expanded));
    } catch {
      // Not persisted; the in-memory state still applies for this session.
    }
  },
};
