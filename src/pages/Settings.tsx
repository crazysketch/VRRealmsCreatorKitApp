import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen, RotateCcw } from "lucide-react";
import { call, pickFolder, type AppConfig, type EngineStatus } from "../lib/api";
import type { Ctx } from "../lib/ctx";

export default function Settings({ ctx }: { ctx: Ctx }) {
  const { t } = useTranslation();
  const { config, overview, run, busy } = ctx;
  const [info, setInfo] = useState<{ version: string; dataDir: string } | null>(null);

  useEffect(() => {
    call<{ version: string; dataDir: string }>("app_info").then(setInfo).catch(() => {});
  }, []);

  const setUpdatesOnStart = async (on: boolean) => {
    await run("settings", () => call<AppConfig>("set_preferences", { checkUpdatesOnStart: on }), () => t("settings.saved"));
  };

  const pickEngine = async () => {
    const dir = await pickFolder(t("settings.enginePick"));
    if (dir) await run("engine", () => call<EngineStatus>("set_engine_path", { path: dir }), () => t("settings.saved"));
  };

  const engine = overview?.engine;

  return (
    <div>
      <h1 className="page-title">{t("settings.title")}</h1>

      <div className="card">
        <div className="card-title mb-8">{t("settings.engine")}</div>
        <div className="path-display mb-8">{engine?.path ?? "—"}</div>
        <div className="text-sm text-muted">
          {config.enginePath ? t("settings.engineManual") : t("settings.engineAuto")}
          {engine?.version ? ` · ${engine.version}` : ""}
        </div>
        <div className="actions">
          <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={pickEngine}>
            <FolderOpen size={13} /> {t("settings.enginePick")}
          </button>
          {config.enginePath && (
            <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={() => run("engine", () => call("set_engine_path", { path: null }))}>
              <RotateCcw size={13} /> {t("settings.engineAutoButton")}
            </button>
          )}
        </div>
      </div>

      <div className="card">
        <div className="card-title mb-8">{t("settings.steamcmd")}</div>
        <div className="path-display mb-8">{overview?.steamcmd.path ?? "—"}</div>
        <div className="text-sm text-muted">{config.steamcmdPath ? "" : t("settings.steamcmdDefault")}</div>
        <div className="setting-row" style={{ marginTop: 8 }}>
          <div>
            <div className="setting-label">{t("settings.steamUser")}</div>
            <div className="setting-desc">{config.steamUser ?? t("settings.steamUserNone")}</div>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="setting-row">
          <div className="setting-label">{t("settings.updatesOnStart")}</div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={config.checkUpdatesOnStart}
              onChange={(e) => setUpdatesOnStart(e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>

      <div className="card">
        <div className="card-title mb-8">{t("settings.about")}</div>
        <div className="status-row">
          <span className="status-label">{t("app.name")}</span>
          <span className="status-value">{info?.version ?? "?"}</span>
        </div>
        <div className="status-row">
          <span className="status-label">{t("settings.kitInUse")}</span>
          <span className="status-value">
            {!overview?.kit?.exists || !overview.kit.version
              ? t("settings.kitInUseNone")
              : overview.kit.legacy
                ? t("settings.kitInUseLegacy", { version: overview.kit.version })
                : overview.kit.version}
          </span>
        </div>
        <div className="status-row">
          <span className="status-label">{t("settings.dataFolder")}</span>
          <span className="path-display" style={{ flex: 1 }}>{info?.dataDir ?? "—"}</span>
        </div>
        {info && (
          <div className="actions">
            <button className="btn btn-secondary btn-sm" onClick={() => call("show_in_explorer", { path: info.dataDir }).catch((e) => ctx.notify("error", String(e)))}>
              <FolderOpen size={13} /> {t("kit.showFolder")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
