import type { AppConfig, Overview } from "./api";

export type Page = "home" | "kit" | "settings" | "help" | "logs";

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
  go: (page: Page) => void;
}
