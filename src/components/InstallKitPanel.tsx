import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, FolderOpen } from "lucide-react";
import { call, pickFolder, type KitInfo } from "../lib/api";
import type { Ctx } from "../lib/ctx";

/** Asks where the new kit goes (C:\VRRealmsCreatorKit by default) and installs it there. */
export default function InstallKitPanel({ ctx, onClose }: { ctx: Ctx; onClose: () => void }) {
  const { t } = useTranslation();
  const [dest, setDest] = useState("");

  useEffect(() => {
    call<string>("suggest_kit_folder", { parent: null }).then(setDest);
  }, []);

  const browse = async () => {
    const parent = await pickFolder(t("kit.installTitle"));
    if (parent) setDest(await call<string>("suggest_kit_folder", { parent }));
  };

  const install = async () => {
    const info = await ctx.run(
      "install-kit",
      () => call<KitInfo>("install_kit", { dest }),
      (k) => t("check.kitOk", { version: k.version ?? "?", root: k.root })
    );
    if (info) onClose();
  };

  return (
    <div className="panel">
      <h3>{t("kit.installTitle")}</h3>
      <div className="field-label">{t("kit.installFolder")}</div>
      <div className="inline-form" style={{ marginTop: 0 }}>
        <input value={dest} onChange={(e) => setDest(e.target.value)} spellCheck={false} />
        <button className="btn btn-secondary" onClick={browse}>
          <FolderOpen size={14} /> {t("kit.browse")}
        </button>
      </div>
      <div className="hint">{t("kit.installHint")}</div>
      <div className="actions">
        <button className="btn btn-accent" disabled={!dest || !!ctx.busy} onClick={install}>
          <Download size={14} /> {ctx.busy === "install-kit" ? t("common.loading") : t("kit.installGo")}
        </button>
        <button className="btn btn-secondary" onClick={onClose}>
          {t("kit.cancel")}
        </button>
      </div>
    </div>
  );
}
