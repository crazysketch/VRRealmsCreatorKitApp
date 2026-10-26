import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, ExternalLink, FolderOpen, Play, RefreshCw, Search, ShieldCheck, Wrench } from "lucide-react";
import InstallKitPanel from "../components/InstallKitPanel";
import {
  call,
  LINKS,
  openLink,
  pickFolder,
  type KitInfo,
  type UpdateCheck,
  type UpdateResult,
  type VerifyResult,
} from "../lib/api";
import type { Ctx } from "../lib/ctx";

function samePath(a: string | null | undefined, b: string | null | undefined) {
  return !!a && !!b && a.replace(/\\+$/, "").toLowerCase() === b.replace(/\\+$/, "").toLowerCase();
}

export default function Kit({ ctx }: { ctx: Ctx }) {
  const { t } = useTranslation();
  const { config, run, busy } = ctx;
  const [kits, setKits] = useState<KitInfo[]>([]);
  const [installing, setInstalling] = useState(false);
  const [found, setFound] = useState<string[] | null>(null);
  const [update, setUpdate] = useState<UpdateCheck | null>(null);
  const [plan, setPlan] = useState<UpdateResult | null>(null);
  const [verify, setVerify] = useState<VerifyResult | null>(null);

  const current = kits.find((k) => samePath(k.root, config.currentKit)) ?? null;

  const loadKits = async () => setKits(await call<KitInfo[]>("list_kits"));
  useEffect(() => {
    loadKits();
  }, [config.kits.length, config.currentKit]);

  useEffect(() => {
    setUpdate(null);
    setPlan(null);
    setVerify(null);
  }, [config.currentKit]);

  const refreshAll = async () => {
    await ctx.refresh();
    await loadKits();
  };

  const checkUpdate = () => {
    ctx.checkApp();
    return (
      current &&
      run("check", () => call<UpdateCheck>("check_kit_update", { path: current.root })).then((r) => r && setUpdate(r))
    );
  };

  // The app updates itself only as an installed copy; Steam updates a Steam copy, a plain .exe is downloaded again.
  const app = ctx.appUpdate;
  const appInstallable = !!app?.hasUpdate && app.how === "installed";
  const installApp = () => run("app-update", () => call("install_app_update"), () => t("kit.appRestarting"));

  const preview = () =>
    current &&
    run("plan", () => call<UpdateResult>("update_kit", { path: current.root, dryRun: true })).then((r) => r && setPlan(r));

  // Opened from a "new update" button: check, then list what the update would change. Nothing changes until Update.
  useEffect(() => {
    if (ctx.intent === "update") ctx.checkApp();
  }, [ctx.intent]);
  useEffect(() => {
    if (ctx.intent !== "update" || !current || busy) return;
    ctx.clearIntent();
    const root = current.root;
    run("check", () => call<UpdateCheck>("check_kit_update", { path: root })).then((r) => {
      if (!r) return;
      setUpdate(r);
      if (r.hasUpdate) run("plan", () => call<UpdateResult>("update_kit", { path: root, dryRun: true })).then((p) => p && setPlan(p));
    });
  }, [ctx.intent, current?.root, busy]);

  const doUpdate = async () => {
    if (!current) return;
    const r = await run("update", () => call<UpdateResult>("update_kit", { path: current.root, dryRun: false }), (x) => x.message);
    if (r) {
      setPlan(null);
      setUpdate(null);
      await loadKits();
      // One update for both: the kit is done, now the app, which closes and opens again as the new version.
      if (appInstallable) await installApp();
    }
  };

  const doVerify = () =>
    current && run("verify", () => call<VerifyResult>("verify_kit", { path: current.root })).then((r) => r && setVerify(r));

  const doRepair = async () => {
    if (!current || !verify) return;
    const files = verify.rows.filter((r) => r.state !== "ok" && !r.yours).map((r) => r.file);
    const done = await run("repair", () => call<string>("repair_kit", { path: current.root, files }), (m) => m);
    if (done) await doVerify();
  };

  const addFolder = async (dir: string | null) => {
    if (!dir) return;
    await run("add-kit", () => call<KitInfo>("add_kit", { path: dir }), (k) => t("check.kitOk", { version: k.version ?? "?", root: k.root }));
    await loadKits();
  };

  const findKits = async () => {
    const r = await run("find", () => call<string[]>("discover_kits"));
    if (r) setFound(r.filter((f) => !kits.some((k) => samePath(k.root, f))));
  };

  const repairable = verify ? verify.rows.filter((r) => r.state !== "ok" && !r.yours).length : 0;

  return (
    <div>
      <h1 className="page-title">{t("kit.title")}</h1>
      <p className="text-sm text-dim mb-16" style={{ maxWidth: 760, lineHeight: 1.5 }}>
        {t("kit.lead")}
      </p>

      {app?.hasUpdate && (
        <div className="card app-update-card">
          <div className="card-header">
            <span className="card-title">{t("kit.appTitle")}</span>
            <span className="badge badge-warning">{t("kit.appAvailable", { latest: app.latest, current: app.current })}</span>
          </div>
          {app.notes && <div className="notes">{app.notes}</div>}
          <div className="hint">{appInstallable ? t("kit.appHow") : t("kit.appHowPortable")}</div>
          <div className="actions">
            {appInstallable ? (
              <button className="btn btn-accent" disabled={!!busy} onClick={installApp}>
                <Download size={14} /> {busy === "app-update" ? t("kit.appUpdating") : t("kit.appUpdate")}
              </button>
            ) : (
              <button className="btn btn-accent" onClick={() => openLink(app.downloadPage)}>
                <ExternalLink size={14} /> {t("kit.appDownload")}
              </button>
            )}
          </div>
        </div>
      )}

      <div className="card">
        <div className="card-header">
          <span className="card-title">{t("kit.yourKits")}</span>
          <button className="btn btn-secondary btn-sm" onClick={refreshAll} disabled={!!busy}>
            <RefreshCw size={13} />
          </button>
        </div>
        {kits.length === 0 && <div className="text-sm text-muted">{t("kit.none")}</div>}
        {kits.map((k) => {
          const isCurrent = samePath(k.root, config.currentKit);
          return (
            <div className="kit-row" key={k.root}>
              <div style={{ minWidth: 0 }}>
                <div className="kit-name">
                  {k.name}
                  {isCurrent && <span className="badge badge-info">{t("kit.current")}</span>}
                  {!k.exists ? (
                    <span className="badge badge-danger">{t("kit.missingFolder")}</span>
                  ) : k.legacy ? (
                    <span className="badge badge-warning">{t("kit.legacy")}</span>
                  ) : (
                    <span className="badge badge-muted">{k.version ?? "?"}</span>
                  )}
                </div>
                <div className="kit-path">{k.root}</div>
              </div>
              <div className="actions" style={{ marginTop: 0, flexShrink: 0 }}>
                {!isCurrent && (
                  <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={() => run("current", () => call("set_current_kit", { path: k.root }))}>
                    {t("kit.makeCurrent")}
                  </button>
                )}
                <button className="btn btn-secondary btn-sm" disabled={!k.exists || !!busy} onClick={() => run("open", () => call("open_kit", { path: k.root }))}>
                  <Play size={13} /> {t("kit.openInUnreal")}
                </button>
                <button className="btn btn-secondary btn-sm" disabled={!k.exists} onClick={() => call("show_in_explorer", { path: k.root }).catch((e) => ctx.notify("error", String(e)))}>
                  <FolderOpen size={13} /> {t("kit.showFolder")}
                </button>
                <button
                  className="btn btn-secondary btn-sm"
                  disabled={!!busy}
                  onClick={async () => {
                    if (window.confirm(t("kit.forgetConfirm"))) {
                      await run("forget", () => call("forget_kit", { path: k.root }));
                      await loadKits();
                    }
                  }}
                >
                  {t("kit.forget")}
                </button>
              </div>
            </div>
          );
        })}
        <div className="actions">
          <button className="btn btn-primary btn-sm" disabled={!!busy} onClick={() => setInstalling(true)}>
            <Download size={13} /> {t("kit.installNew")}
          </button>
          <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={async () => addFolder(await pickFolder(t("kit.addExisting")))}>
            <FolderOpen size={13} /> {t("kit.addExisting")}
          </button>
          <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={findKits}>
            <Search size={13} /> {busy === "find" ? t("kit.searching") : t("kit.findOnPc")}
          </button>
        </div>
        {installing && (
          <InstallKitPanel
            ctx={ctx}
            onClose={() => {
              setInstalling(false);
              loadKits();
            }}
          />
        )}
        {found && (
          <div className="panel">
            <h3>{t("kit.found")}</h3>
            {found.length === 0 && <div className="text-sm text-muted">{t("kit.foundNone")}</div>}
            {found.map((f) => (
              <div className="file-row" key={f}>
                <span className="font-mono">{f}</span>
                <button
                  className="btn btn-secondary btn-sm"
                  disabled={!!busy}
                  onClick={async () => {
                    await addFolder(f);
                    setFound((list) => (list ?? []).filter((x) => x !== f));
                  }}
                >
                  {t("kit.add")}
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {current?.exists && (
        <div className="card">
          <div className="card-header">
            <span className="card-title">
              {t("kit.updates")} · {current.name}
            </span>
          </div>
          <div className="actions" style={{ marginTop: 0 }}>
            <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={checkUpdate}>
              <RefreshCw size={13} className={busy === "check" ? "spin" : ""} /> {t("kit.check")}
            </button>
            <button className="btn btn-secondary btn-sm" disabled={!!busy} onClick={doVerify}>
              <ShieldCheck size={13} /> {busy === "verify" ? t("kit.verifying") : t("kit.verify")}
            </button>
          </div>

          {update && (
            <div className="mt-12">
              {update.hasUpdate ? (
                <span className="badge badge-warning">
                  {t("kit.available", { latest: update.latest, installed: update.installed ?? "?" })}
                </span>
              ) : (
                <span className="badge badge-success">{t("kit.upToDate", { version: update.installed ?? update.latest })}</span>
              )}
              {update.notes && (
                <>
                  <div className="card-title mt-12">{t("kit.whatsNew")}</div>
                  <div className="notes">{update.notes.slice(0, 3000)}</div>
                </>
              )}
              {update.hasUpdate && !plan && (
                <div className="actions">
                  <button className="btn btn-accent" disabled={!!busy} onClick={preview}>
                    <Download size={14} /> {t("kit.update")}
                  </button>
                </div>
              )}
            </div>
          )}

          {plan && (
            <div className="panel">
              <h3>{t("kit.planTitle")}</h3>
              {plan.plan.pluginFolderOnly && <div className="hint">{t("kit.planPluginOnly")}</div>}
              <div className="text-sm">{t("kit.planSummary", { replace: plan.plan.replace.length, add: plan.plan.add.length })}</div>
              {plan.plan.keptYours.length > 0 && (
                <>
                  <div className="text-sm text-dim mt-8">{t("kit.planKept", { count: plan.plan.keptYours.length })}</div>
                  <div className="file-list">
                    {plan.plan.keptYours.map((f) => (
                      <div className="file-row" key={f}>
                        <span className="font-mono">{f}</span>
                      </div>
                    ))}
                  </div>
                </>
              )}
              {ctx.overview?.editorRunning && <div className="hint" style={{ color: "var(--warn)" }}>{t("kit.closeEditor")}</div>}
              <div className="actions">
                <button className="btn btn-accent" disabled={!!busy || ctx.overview?.editorRunning || plan.plan.replace.length + plan.plan.add.length === 0} onClick={doUpdate}>
                  <Download size={14} /> {busy === "update" ? t("kit.updating") : appInstallable ? t("kit.planConfirmBoth") : t("kit.planConfirm")}
                </button>
                <button className="btn btn-secondary" onClick={() => setPlan(null)}>
                  {t("kit.cancel")}
                </button>
              </div>
            </div>
          )}

          {verify && (
            <div className="panel">
              <div className="text-sm">
                {verify.modified + verify.missing === 0
                  ? t("kit.verifyAllOk", { count: verify.rows.length, version: verify.version })
                  : t("kit.verifySome", {
                      bad: verify.modified + verify.missing,
                      count: verify.rows.length,
                      modified: verify.modified,
                      missing: verify.missing,
                    })}
              </div>
              <div className="file-list">
                {verify.rows.map((r) => (
                  <div className="file-row" key={r.file}>
                    <span className={`badge ${r.state === "ok" ? "badge-success" : r.state === "missing" ? "badge-danger" : "badge-warning"}`}>
                      {r.state === "ok" ? t("kit.stateOk") : r.state === "missing" ? t("kit.stateMissing") : t("kit.stateModified")}
                    </span>
                    <span className="font-mono">{r.file}</span>
                    {r.yours && <span className="badge badge-muted">{t("kit.yours")}</span>}
                  </div>
                ))}
              </div>
              {repairable > 0 && (
                <div className="actions">
                  <button className="btn btn-primary" disabled={!!busy || ctx.overview?.editorRunning} onClick={doRepair}>
                    <Wrench size={14} /> {busy === "repair" ? t("kit.repairing") : t("kit.repair")}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      )}

      <div className="card">
        <button className="link-btn" onClick={() => openLink(LINKS.kitReleases)}>
          {t("kit.zipWay")} <ExternalLink size={12} />
        </button>
        <div className="hint">{t("kit.zipWayHint")}</div>
      </div>
    </div>
  );
}
