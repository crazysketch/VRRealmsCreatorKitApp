import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import Sidebar from "./components/Sidebar";
import ProgressCard from "./components/ProgressCard";
import Home from "./pages/Home";
import Kit from "./pages/Kit";
import Settings from "./pages/Settings";
import Help from "./pages/Help";
import Logs from "./pages/Logs";
import { call, onEvent, onProgress, type AppConfig, type Overview, type Progress } from "./lib/api";
import type { Ctx, Page } from "./lib/ctx";

type Msg = { kind: "ok" | "error" | "info"; text: string } | null;

export default function App() {
  const { t } = useTranslation();
  const [page, setPage] = useState<Page>("home");
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState<Msg>(null);
  const busyRef = useRef("");

  const refresh = useCallback(async () => {
    try {
      const [cfg, ov] = await Promise.all([call<AppConfig>("get_config"), call<Overview>("get_overview")]);
      setConfig(cfg);
      setOverview(ov);
    } catch (e) {
      setMsg({ kind: "error", text: String(e) });
    }
  }, []);

  const notify = useCallback((kind: "ok" | "error" | "info", text: string) => setMsg({ kind, text }), []);

  const run = useCallback(
    async <T,>(key: string, job: () => Promise<T>, okText?: (r: T) => string | null): Promise<T | undefined> => {
      if (busyRef.current) return undefined;
      busyRef.current = key;
      setBusy(key);
      setMsg(null);
      try {
        const result = await job();
        const text = okText?.(result);
        if (text) setMsg({ kind: "ok", text });
        return result;
      } catch (e) {
        setMsg({ kind: "error", text: String(e) });
        return undefined;
      } finally {
        busyRef.current = "";
        setBusy("");
        setProgress(null);
        await refresh();
      }
    },
    [refresh]
  );

  useEffect(() => {
    refresh();
    const unlisten: Array<() => void> = [];
    onProgress((p) => setProgress(p)).then((u) => unlisten.push(u));
    onEvent("epic-installer-closed", () => refresh()).then((u) => unlisten.push(u));
    // Things get installed outside the app (Epic's launcher, Steam): look again when the window comes back.
    const onFocus = () => {
      if (!busyRef.current) refresh();
    };
    window.addEventListener("focus", onFocus);
    const onLinkFailed = () => setMsg({ kind: "error", text: t("common.linkFailed") });
    window.addEventListener("vrr-link-failed", onLinkFailed);
    return () => {
      unlisten.forEach((u) => u());
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("vrr-link-failed", onLinkFailed);
    };
  }, [refresh, t]);

  if (!config) {
    return (
      <div className="app-layout" style={{ alignItems: "center", justifyContent: "center" }}>
        <div className="text-muted">{t("common.loading")}</div>
      </div>
    );
  }

  const ctx: Ctx = { config, overview, refresh, run, busy, notify, go: setPage };

  return (
    <div className="app-layout">
      <Sidebar current={page} onNavigate={setPage} />
      <main className="main-content">
        {progress && busy && <ProgressCard progress={progress} />}
        {msg && (
          <div className={`message ${msg.kind}`}>
            <span>{msg.text}</span>
            <button onClick={() => setMsg(null)} aria-label={t("common.close")}>
              <X size={14} />
            </button>
          </div>
        )}
        {page === "home" && <Home ctx={ctx} />}
        {page === "kit" && <Kit ctx={ctx} />}
        {page === "settings" && <Settings ctx={ctx} />}
        {page === "help" && <Help />}
        {page === "logs" && <Logs ctx={ctx} />}
      </main>
    </div>
  );
}
