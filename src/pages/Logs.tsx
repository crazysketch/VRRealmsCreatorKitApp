import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen, RefreshCw } from "lucide-react";
import { call } from "../lib/api";
import type { Ctx } from "../lib/ctx";

export default function Logs({ ctx }: { ctx: Ctx }) {
  const { t } = useTranslation();
  const [log, setLog] = useState<{ lines: string[]; file: string | null } | null>(null);

  const load = () => call<{ lines: string[]; file: string | null }>("get_log").then(setLog);
  useEffect(() => {
    load();
  }, []);

  return (
    <div>
      <div className="flex justify-between items-center mb-8">
        <h1 className="page-title" style={{ marginBottom: 0 }}>
          {t("logs.title")}
        </h1>
        <div className="actions" style={{ marginTop: 0 }}>
          <button className="btn btn-secondary btn-sm" onClick={load}>
            <RefreshCw size={13} /> {t("logs.refresh")}
          </button>
          {log?.file && (
            <button className="btn btn-secondary btn-sm" onClick={() => call("show_in_explorer", { path: log.file }).catch((e) => ctx.notify("error", String(e)))}>
              <FolderOpen size={13} /> {t("logs.showFile")}
            </button>
          )}
        </div>
      </div>
      <p className="text-sm text-muted mb-12">{t("logs.lead")}</p>
      <div className="log-box">{log && log.lines.length > 0 ? log.lines.join("\n") : t("logs.empty")}</div>
    </div>
  );
}
