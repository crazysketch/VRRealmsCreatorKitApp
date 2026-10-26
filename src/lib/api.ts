// Talks to the app's Rust side. Opened in a plain browser (to look at the design), it answers from
// the sample data at the bottom instead, so every page can be seen without the app.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

export interface AppConfig {
  version: number;
  language: string;
  kits: string[];
  currentKit: string | null;
  enginePath: string | null;
  steamcmdPath: string | null;
  steamUser: string | null;
  checkUpdatesOnStart: boolean;
}

export interface EngineStatus {
  state: "ok" | "wrong_version" | "missing";
  path: string | null;
  version: string | null;
  editorExe: string | null;
  source: string | null;
}

export interface KitInfo {
  root: string;
  name: string;
  exists: boolean;
  version: string | null;
  legacy: boolean;
  contentVersion: string | null;
  uproject: string;
}

export interface Overview {
  engine: EngineStatus;
  epic: { installed: boolean; exe: string | null };
  steam: { installed: boolean; running: boolean; signedIn: boolean; vrRealmsInstalled: boolean };
  steamcmd: { installed: boolean; path: string };
  kit: KitInfo | null;
  editorRunning: boolean;
  freeBytes: number | null;
}

export interface UpdateCheck {
  installed: string | null;
  latest: string;
  hasUpdate: boolean;
  notes: string;
  publishedAt: string | null;
  releasePage: string;
}

export interface UpdatePlan {
  from: string;
  to: string;
  replace: string[];
  add: string[];
  keptYours: string[];
  pluginFolderOnly: boolean;
}

export interface UpdateResult {
  plan: UpdatePlan;
  backupFolder: string | null;
  message: string;
}

export interface VerifyRow {
  file: string;
  state: "ok" | "modified" | "missing";
  yours: boolean;
}

export interface VerifyResult {
  version: string;
  rows: VerifyRow[];
  ok: number;
  modified: number;
  missing: number;
}

export interface NewsItem {
  title: string;
  date: string;
  body: string;
  url?: string | null;
  tag?: string | null;
}

export interface Progress {
  task: string;
  label: string;
  done: number;
  total: number | null;
}

export const LINKS = {
  discord: "https://discord.com/invite/qMZ7gZzg6A",
  docs: "https://vr-realms.com/docs/ugc-tools.html",
  website: "https://vr-realms.com",
  kitReleases: "https://github.com/crazysketch/VRRealmsCreatorKit/releases/latest",
  appLicense: "https://github.com/crazysketch/VRRealmsCreatorKitApp/blob/main/LICENSE.md",
  appNotices: "https://github.com/crazysketch/VRRealmsCreatorKitApp/blob/main/THIRD-PARTY-NOTICES.md",
  steamStore: "https://store.steampowered.com/app/2964550/VR_Realms/",
};

export const inApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inApp) return tauriInvoke<T>(cmd, args);
  return sample<T>(cmd);
}

export async function onProgress(fn: (p: Progress) => void): Promise<() => void> {
  if (!inApp) return () => {};
  return tauriListen<Progress>("progress", (e) => fn(e.payload));
}

export async function onEvent(name: string, fn: () => void): Promise<() => void> {
  if (!inApp) return () => {};
  return tauriListen(name, () => fn());
}

/** Opens a link in the browser (or Steam). A link the app may not open is reported, never thrown. */
export async function openLink(url: string) {
  try {
    if (!inApp) {
      window.open(url, "_blank", "noopener");
      return;
    }
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch (e) {
    window.dispatchEvent(new CustomEvent("vrr-link-failed", { detail: String(e) }));
  }
}

export async function pickFolder(title: string): Promise<string | null> {
  if (!inApp) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, title });
  return typeof picked === "string" ? picked : null;
}

export function formatBytes(n: number | null | undefined): string {
  if (n == null) return "?";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

// ----- sample data for looking at the design in a browser ---------------------------------------

const SAMPLE_KIT: KitInfo = {
  root: "C:\\VRRealmsCreatorKit",
  name: "VRRealmsCreatorKit",
  exists: true,
  version: "0.4.32",
  legacy: false,
  contentVersion: "1",
  uproject: "C:\\VRRealmsCreatorKit\\VRRealms\\VRRealms.uproject",
};

function sample<T>(cmd: string): Promise<T> {
  const data: Record<string, unknown> = {
    get_config: {
      version: 1,
      language: "en",
      kits: [SAMPLE_KIT.root],
      currentKit: SAMPLE_KIT.root,
      enginePath: null,
      steamcmdPath: null,
      steamUser: null,
      checkUpdatesOnStart: true,
    },
    get_overview: {
      engine: { state: "ok", path: "C:\\Program Files\\Epic Games\\UE_5.8", version: "5.8.3", editorExe: "UnrealEditor.exe", source: "Epic Games Launcher" },
      epic: { installed: true, exe: "EpicGamesLauncher.exe" },
      steam: { installed: true, running: true, signedIn: true, vrRealmsInstalled: true },
      steamcmd: { installed: false, path: "C:\\steamcmd\\steamcmd.exe" },
      kit: SAMPLE_KIT,
      editorRunning: false,
      freeBytes: 412 * 1024 ** 3,
    },
    list_kits: [SAMPLE_KIT],
    check_kit_update: {
      installed: "0.4.31",
      latest: "0.4.32",
      hasUpdate: true,
      notes: "Import avatar file: bring in .zip, .unitypackage, .vrm and .fbx avatars.\nMetaHuman packages (.mhpkg).",
      publishedAt: "2026-10-21T22:07:17Z",
      releasePage: LINKS.kitReleases,
    },
    get_news: {
      source: "website",
      items: [
        { title: "Thanksgiving World Contest", date: "2026-11-01", body: "Build a Thanksgiving world Nov 1 to 18, players vote Nov 19 to 26. 3,000 / 2,000 / 1,000 gold for the top three, 250 for every other world.", url: "https://vr-realms.com/contest", tag: "Event" },
        { title: "Creator Kit 0.4.32", date: "2026-10-21", body: "Import avatar file: .zip, .unitypackage, .vrm and .fbx avatars come in with one button.", url: LINKS.kitReleases, tag: "Kit" },
        { title: "Saturday Hangout", date: "2026-10-31", body: "Every Saturday at 8 AM Pacific in the hub.", url: "https://vr-realms.com/events.php", tag: "Event" },
      ],
    },
    get_log: { lines: ["2026-10-26 11:02:10 [info] VR Realms Creator Kit 0.4.32 started"], file: "C:\\Users\\you\\AppData\\Roaming\\com.vrrealms.creatorkit\\logs\\app.log" },
    app_info: { version: "0.4.32", dataDir: "C:\\Users\\you\\AppData\\Roaming\\com.vrrealms.creatorkit" },
    discover_kits: [SAMPLE_KIT.root],
    suggest_kit_folder: "C:\\VRRealmsCreatorKit",
  };
  return new Promise((resolve) => setTimeout(() => resolve(data[cmd] as T), 150));
}
