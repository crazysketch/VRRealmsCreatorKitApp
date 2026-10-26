import { useTranslation } from "react-i18next";
import { BookOpen, Boxes, FileText, Home, LifeBuoy, Settings, Sparkles, type LucideIcon } from "lucide-react";
import { LINKS, openLink } from "../lib/api";
import type { Page } from "../lib/ctx";
import DiscordIcon from "./DiscordIcon";
import pkg from "../../package.json";

const items: { id: Page; icon: LucideIcon; label: string }[] = [
  { id: "home", icon: Home, label: "nav.home" },
  { id: "creations", icon: Sparkles, label: "nav.creations" },
  { id: "kit", icon: Boxes, label: "nav.kit" },
  { id: "settings", icon: Settings, label: "nav.settings" },
  { id: "help", icon: LifeBuoy, label: "nav.help" },
  { id: "logs", icon: FileText, label: "nav.logs" },
];

export default function Sidebar({
  current,
  onNavigate,
  badges,
}: {
  current: Page;
  onNavigate: (p: Page) => void;
  /** A small tag after a page's name, such as NEW on the Creator Kit while an update is out. */
  badges?: Partial<Record<Page, string>>;
}) {
  const { t } = useTranslation();
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="brand">
          <img src="/icon.png" alt="" />
          <div>
            <h1>{t("app.name")}</h1>
            <div className="version">{t("app.version", { version: pkg.version })}</div>
          </div>
        </div>
      </div>
      <nav className="sidebar-nav">
        {items.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              className={`nav-item ${current === item.id ? "active" : ""}`}
              onClick={() => onNavigate(item.id)}
            >
              <Icon />
              {t(item.label)}
              {badges?.[item.id] && <span className="nav-badge">{badges[item.id]}</span>}
            </button>
          );
        })}
      </nav>
      <div className="sidebar-footer">
        <button className="sidebar-discord" onClick={() => openLink(LINKS.discord)}>
          <DiscordIcon size={16} />
          {t("nav.discord")}
        </button>
        <div className="sidebar-hint">{t("nav.discordHint")}</div>
        <button className="link" onClick={() => openLink(LINKS.docs)}>
          <BookOpen size={14} />
          {t("nav.docs")}
        </button>
      </div>
    </aside>
  );
}
