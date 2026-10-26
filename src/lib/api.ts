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

/** The app's own update: "installed" copies update themselves, "steam" ones are Steam's to update, "portable" ones are downloaded again. */
export interface AppUpdate {
  current: string;
  latest: string | null;
  hasUpdate: boolean;
  notes: string;
  how: "installed" | "steam" | "portable";
  downloadPage: string;
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

export interface LiveEvent {
  title: string;
  startsAt: number;
  minutes: number;
  world: string;
  live: boolean;
}

export interface LivePoll {
  question: string;
  votes: number | null;
  closesAt: number | null;
  url: string | null;
}

export interface LiveContest {
  title: string;
  phase: "upcoming" | "building" | "voting" | "closed";
  buildFrom: number;
  buildUntil: number;
  voteFrom: number;
  voteUntil: number;
  prizes: number[];
  entryGold: number;
  entries: number;
  url: string | null;
}

/** VR Realms right now, from vr-realms.com (counts only). */
export interface Live {
  online: number | null;
  hub: number | null;
  events: LiveEvent[];
  polls: LivePoll[];
  contest: LiveContest | null;
}

export type InGame = "approved" | "update_waiting" | "not_verified" | "waiting" | "blocked" | "unknown";

export interface Creation {
  id: string;
  kind: "world" | "avatar" | "other";
  title: string;
  preview: string;
  updated: number;
  tags: string[];
  votesUp: number;
  votesDown: number;
  comments: number;
  subscribers: number;
  favorites: number;
  inGame: InGame;
  players: number | null;
  playersWeek: number | null;
  playingNow: number | null;
  visits: number | null;
  minutes: number | null;
  url: string | null;
  commentsUrl: string | null;
}

export interface Creations {
  hasAccount: boolean;
  signedInNow: boolean;
  name: string | null;
  items: Creation[];
  workshopUrl: string | null;
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
  playGame: "steam://run/2964550",
  polls: "https://vr-realms.com/polls.php",
  events: "https://vr-realms.com/events.php",
  buildWorld: "https://vr-realms.com/docs/maps-build.html",
  buildAvatar: "https://vr-realms.com/docs/workshop-avatars.html",
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

const SAMPLE_CREATIONS: Creation[] = (
  [
    ["3756607915", "world", "Jog at the Park", "9736855058979609560/0412F58E356ADB2EBC4786F7B240F24FCD3C7307", 1792122871, ["Game World", "Large (32+)", "Social Hub"], 0, 9, "approved", 79, 20, 2, 220, 557],
    ["3759363211", "world", "The Night Club", "10503068880891783465/E06F8976A887935809FEBA28C28306B974CA6B36", 1790382655, ["Large (32+)", "Social Hub"], 0, 19, "approved", 126, 10, 0, 382, 640],
    ["3797187408", "world", "Boxing Royal", "17562683215595931003/E3454D220F1EF84AE799ED9DCFB40482236D0AB2", 1792316400, ["Game World", "Minigame"], 0, 4, "update_waiting", 38, 8, 0, 116, 240],
    ["3780615754", "avatar", "Mechanical Girl", "13573250029654093973/2C2BA3D57CD946881461A01D654FCE7054324C37", 1786298402, ["Humanoid", "Fantasy"], 2, 23, "approved", 92, 3, 1, null, 1582],
    ["3819527362", "avatar", "Vampire Girl", "16493726572891037159/E5B3AD748F839C1D5A6D2CD202955ACC2745E4B7", 1791958637, ["Humanoid", "Creature", "Blend Shapes"], 0, 7, "approved", 35, 20, 0, null, 339],
    ["3823310063", "avatar", "Capricorn", "11135145990399097547/7309097AEC7900A9AED8BCF150FC8FA41450573A", 1792473111, ["Realistic"], 0, 0, "waiting", 4, 4, 0, null, 7],
  ] as const
).map(([id, kind, title, pic, updated, tags, up, subs, inGame, players, week, now, visits, minutes]) => ({
  id, kind, title, preview: `https://images.steamusercontent.com/ugc/${pic}/`, updated, tags: [...tags], votesUp: up, votesDown: 0,
  comments: 0, subscribers: subs, favorites: 0, inGame, players, playersWeek: week, playingNow: now, visits, minutes,
  url: `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`, commentsUrl: `https://steamcommunity.com/sharedfiles/filedetails/comments/${id}`,
}));

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
      installed: "0.4.32",
      latest: "0.5.1",
      hasUpdate: true,
      notes: "Import avatar file: bring in .zip, .unitypackage, .vrm and .fbx avatars.\nMetaHuman packages (.mhpkg).",
      publishedAt: "2026-10-21T22:07:17Z",
      releasePage: LINKS.kitReleases,
    },
    get_news: {
      source: "website",
      items: [
        { title: "Thanksgiving World Contest", date: "2026-11-01", body: "Build a Thanksgiving world Nov 1 to 18, players vote Nov 19 to 26. 3,000 / 2,000 / 1,000 gold for the top three, 250 for every other world.", url: "https://vr-realms.com/contest", tag: "Event" },
        { title: "Creator Kit 0.5.1", date: "2026-10-26", body: "The Creator Kit app sets everything up for you, and imported avatars take far less disk space.", url: LINKS.kitReleases, tag: "Kit" },
        { title: "Saturday Hangout", date: "2026-10-31", body: "Every Saturday at 8 AM Pacific in the hub.", url: "https://vr-realms.com/events.php", tag: "Event" },
      ],
    },
    check_app_update: {
      current: "0.5.1",
      latest: "0.5.2",
      hasUpdate: true,
      notes: "The app now updates itself together with the kit.",
      how: "installed",
      downloadPage: "https://github.com/crazysketch/VRRealmsCreatorKitApp/releases/latest",
    },
    get_live: {
      online: 7,
      hub: 4,
      events: [
        { title: "Saturday Hangout", startsAt: 1793458800, minutes: 120, world: "THE HUB", live: false },
        { title: "Halloween Event", startsAt: 1793475000, minutes: 60, world: "THE HUB", live: false },
      ],
      polls: [
        { question: "How do you like Home Loft, the home you can decorate?", votes: null, closesAt: 1793606340, url: "https://vr-realms.com/polls.php#poll-9" },
        { question: "What should VR Realms do more of next?", votes: null, closesAt: 1793606340, url: "https://vr-realms.com/polls.php#poll-7" },
      ],
      contest: {
        title: "Thanksgiving World Contest", phase: "upcoming", buildFrom: 1793516400, buildUntil: 1795075199, voteFrom: 1795075200,
        voteUntil: 1795766399, prizes: [3000, 2000, 1000], entryGold: 250, entries: 0, url: "https://vr-realms.com/contest",
      },
    },
    get_creations: {
      hasAccount: true,
      signedInNow: true,
      name: "CrazySketch",
      workshopUrl: "https://steamcommunity.com/profiles/76561197963309864/myworkshopfiles/?appid=2964550",
      items: typeof location !== "undefined" && location.search.includes("empty") ? [] : SAMPLE_CREATIONS,
    },
    get_log: { lines: ["2026-10-26 11:02:10 [info] VR Realms Creator Kit 0.5.1 started"], file: "C:\\Users\\you\\AppData\\Roaming\\com.vrrealms.creatorkit\\logs\\app.log" },
    app_info: { version: "0.5.1", dataDir: "C:\\Users\\you\\AppData\\Roaming\\com.vrrealms.creatorkit" },
    discover_kits: [SAMPLE_KIT.root],
    suggest_kit_folder: "C:\\VRRealmsCreatorKit",
  };
  return new Promise((resolve) => setTimeout(() => resolve(data[cmd] as T), 150));
}
