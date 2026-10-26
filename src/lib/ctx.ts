import type { AppConfig, AppUpdate, Overview, UpdateCheck } from "./api";

export type Page = "home" | "creations" | "kit" | "settings" | "help" | "logs";

/** What a page was opened for: "update" opens the Creator Kit page with the update checked and its changes listed. */
export type Intent = "update" | null;

/** What every page gets from the app frame. */
export interface Ctx {
  config: AppConfig;
  overview: Overview | null;
  /** Reads settings and the checklist again. */
  refresh: () => Promise<void>;
  /** Runs a job: one at a time, errors shown as a message, the checklist read again after. */
  run: <T>(key: string, job: () => Promise<T>, okText?: (result: T) => string | null) => Promise<T | undefined>;
  /** The job running now ("" when none). */
  busy: string;
  notify: (kind: "ok" | "error" | "info", text: string) => void;
  go: (page: Page, intent?: Intent) => void;
  intent: Intent;
  clearIntent: () => void;
  /** The current kit's update check (null until it answers, or when checking at start is off). */
  kitUpdate: UpdateCheck | null;
  /** The app's own update check (null until it answers). */
  appUpdate: AppUpdate | null;
  /** Asks again whether the app has an update. */
  checkApp: () => Promise<AppUpdate | null>;
}
