import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, ExternalLink, FolderOpen, LogIn, Play, RefreshCw, Sparkles } from "lucide-react";
import CheckRow, { type CheckState } from "../components/CheckRow";
import InstallKitPanel from "../components/InstallKitPanel";
import LiveCard from "../components/LiveCard";
import NewsCard from "../components/NewsCard";
import { call, formatBytes, LINKS, openLink, pickFolder, type KitInfo } from "../lib/api";
import type { Ctx } from "../lib/ctx";

const LOW_SPACE = 80 * 1024 ** 3;

export default function Home({ ctx }: { ctx: Ctx }) {
  const { t } = useTranslation();
  const { overview: ov, config, run, busy } = ctx;
  const [installing, setInstalling] = useState(false);
  const [user, setUser] = useState(config.steamUser ?? "");

  const kit = ov?.kit ?? null;
  const update = ctx.kitUpdate;
  const kitNew = !!kit?.exists && !!update?.hasUpdate;
  const appNew = !!ctx.appUpdate?.hasUpdate;

  if (!ov) return <div className="text-muted">{t("common.loading")}</div>;

  const engineOk = ov.engine.state === "ok";
  const kitOk = !!kit?.exists;
  const ready = engineOk && kitOk;

  // ----- rows -----
  const epicState: CheckState = ov.epic.installed ? "ok" : "bad";
  const ueState: CheckState = engineOk ? "ok" : ov.engine.state === "wrong_version" ? "warn" : "bad";
  const kitState: CheckState = !kitOk ? "bad" : update?.hasUpdate ? "warn" : "ok";
  const steamState: CheckState = ov.steam.signedIn && ov.steam.vrRealmsInstalled ? "ok" : ov.steam.installed ? "warn" : "bad";
  const cmdState: CheckState = ov.steamcmd.installed ? "ok" : "bad";
  const loginState: CheckState = config.steamUser ? "ok" : "warn";

  const space =
    !engineOk && ov.freeBytes != null ? (
      <div className={ov.freeBytes < LOW_SPACE ? "" : "text-muted"} style={{ marginTop: 4, color: ov.freeBytes < LOW_SPACE ? "var(--warn)" : undefined }}>
        {ov.freeBytes < LOW_SPACE ? t("check.spaceLow", { free: formatBytes(ov.freeBytes) }) : t("check.space", { free: formatBytes(ov.freeBytes) })}
      </div>
    ) : null;

  const pickEngine = async () => {
    const dir = await pickFolder(t("settings.enginePick"));
    if (dir) await run("engine", () => call("set_engine_path", { path: dir }));
  };

  const addMine = async () => {
    const dir = await pickFolder(t("check.kitAddMine"));
    if (dir) await run("add-kit", () => call<KitInfo>("add_kit", { path: dir }), (k) => t("check.kitOk", { version: k.version ?? "?", root: k.root }));
  };

  return (
    <div>
      <div className="hero">
        <div>
          <h1>{t("home.title")}</h1>
          <p>{t("home.lead")}</p>
        </div>
        <div className="hero-side">
          <div className="hero-buttons">
            {(kitNew || appNew) && (
              <button className="btn btn-update btn-xl" onClick={() => ctx.go("kit", "update")} title={t("home.updateHint")}>
                <span className="update-ping" />
                <Sparkles size={17} />
                <span className="update-label">
                  <span className="update-kicker">{t("home.updateKicker")}</span>
                  {kitNew && appNew
                    ? t("home.updateBoth", { latest: update!.latest })
                    : kitNew
                    ? t("home.updateButton", { latest: update!.latest })
                    : t("home.updateApp", { latest: ctx.appUpdate!.latest })}
                </span>
              </button>
            )}
            <button
              className="btn btn-accent btn-xl"
              disabled={!ready || !!busy}
              onClick={() => run("open", () => call("open_kit", { path: null }))}
            >
              <Play size={17} /> {busy === "open" ? t("home.openBusy") : t("home.open")}
            </button>
          </div>
          {!ready && <span className="note">{t("home.notReady")}</span>}
          {ready && ov.editorRunning && <span className="note">{t("home.editorRunning")}</span>}
        </div>
      </div>

      <div className="home-grid">
        <div className="card">
          <div className="card-header">
            <span className="card-title">{t("home.checklist")}</span>
            <button className="btn btn-secondary btn-sm" onClick={() => ctx.refresh()} disabled={!!busy}>
              <RefreshCw size={13} />
            </button>
          </div>
          <div className="card-lead">{t("home.checklistLead")}</div>

          <CheckRow state={epicState} title={t("check.epicTitle")} text={ov.epic.installed ? t("check.epicOk") : t("check.epicMissing")}>
            {!ov.epic.installed && (
              <button
                className="btn btn-primary btn-sm"
                disabled={!!busy}
                onClick={() => run("epic", () => call("install_epic_launcher"), () => t("check.epicStarted"))}
              >
                <Download size={13} /> {busy === "epic" ? t("check.epicInstalling") : t("check.epicInstall")}
              </button>
            )}
          </CheckRow>

          <CheckRow
            state={ueState}
            title={t("check.ueTitle")}
            text={
              <>
                {engineOk
                  ? t("check.ueOk", { version: ov.engine.version, source: ov.engine.source })
                  : ov.engine.state === "wrong_version"
                  ? t("check.ueWrong", { version: ov.engine.version })
                  : t("check.ueMissing")}
                {space}
              </>
            }
          >
            {!engineOk && (
              <>
                <button
                  className="btn btn-primary btn-sm"
                  disabled={!ov.epic.installed || !!busy}
                  onClick={() => run("ue", () => call("open_unreal_page"))}
                >
                  <ExternalLink size={13} /> {t("check.ueOpenEpic")}
                </button>
                <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={pickEngine}>
                  <FolderOpen size={13} /> {t("check.uePick")}
                </button>
              </>
            )}
          </CheckRow>

          <CheckRow
            state={kitState}
            title={t("check.kitTitle")}
            text={
              !kitOk
                ? t("check.kitMissing")
                : update?.hasUpdate
                ? t("check.kitUpdate", { version: kit!.version ?? "?", latest: update.latest })
                : t("check.kitOk", { version: kit!.version ?? "?", root: kit!.root })
            }
          >
            {!kitOk && !installing && (
              <>
                <button className="btn btn-primary btn-sm" disabled={!!busy} onClick={() => setInstalling(true)}>
                  <Download size={13} /> {t("check.kitInstall")}
                </button>
                <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={addMine}>
                  <FolderOpen size={13} /> {t("check.kitAddMine")}
                </button>
              </>
            )}
            {kitOk && update?.hasUpdate && (
              <button className="btn btn-primary btn-sm" onClick={() => ctx.go("kit", "update")}>
                <Download size={13} /> {t("check.kitUpdateNow", { latest: update.latest })}
              </button>
            )}
          </CheckRow>
          {installing && <InstallKitPanel ctx={ctx} onClose={() => setInstalling(false)} />}

          <CheckRow
            state={steamState}
            title={t("check.steamTitle")}
            text={
              !ov.steam.installed
                ? t("check.steamMissing")
                : !ov.steam.signedIn
                ? t("check.steamNotSignedIn")
                : ov.steam.vrRealmsInstalled
                ? t("check.steamOk")
                : t("check.steamNoGame")
            }
          >
            {ov.steam.installed && !ov.steam.signedIn && (
              <button className="btn btn-secondary btn-sm" onClick={() => openLink("steam://open/main")}>
                {t("check.steamOpen")}
              </button>
            )}
            {ov.steam.signedIn && !ov.steam.vrRealmsInstalled && (
              <button className="btn btn-primary btn-sm" onClick={() => openLink("steam://install/2964550")}>
                <Download size={13} /> {t("check.steamGetGame")}
              </button>
            )}
            {!ov.steam.installed && (
              <button className="btn btn-secondary btn-sm" onClick={() => openLink(LINKS.steamStore)}>
                <ExternalLink size={13} /> {t("check.steamGetGame")}
              </button>
            )}
          </CheckRow>

          <CheckRow
            state={cmdState}
            title={t("check.steamcmdTitle")}
            text={ov.steamcmd.installed ? t("check.steamcmdOk", { path: ov.steamcmd.path }) : t("check.steamcmdMissing")}
          >
            {!ov.steamcmd.installed && (
              <button
                className="btn btn-primary btn-sm"
                disabled={!!busy}
                onClick={() => run("steamcmd", () => call<string>("install_steamcmd", { dir: null }), (p) => t("check.steamcmdOk", { path: p }))}
              >
                <Download size={13} /> {busy === "steamcmd" ? t("check.steamcmdInstalling") : t("check.steamcmdInstall")}
              </button>
            )}
          </CheckRow>

          <CheckRow
            state={loginState}
            title={t("check.loginTitle")}
            text={config.steamUser ? t("check.loginSaved", { user: config.steamUser }) : t("check.loginLead")}
          >
            {null}
          </CheckRow>
          <div className="inline-form" style={{ marginLeft: 34, marginBottom: 6 }}>
            <input
              placeholder={t("check.loginPlaceholder")}
              value={user}
              onChange={(e) => setUser(e.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
            <button
              className="btn btn-secondary"
              disabled={!user.trim() || !ov.steamcmd.installed || !!busy}
              onClick={() => run("login", () => call<string>("steam_login", { user }), (m) => m)}
            >
              <LogIn size={14} /> {config.steamUser ? t("check.loginAgain") : t("check.loginButton")}
            </button>
          </div>
        </div>

        <div>
          <LiveCard />
          <NewsCard />
        </div>
      </div>
    </div>
  );
}
