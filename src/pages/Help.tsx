import { useTranslation } from "react-i18next";
import { BookOpen, ExternalLink, FileText, Globe } from "lucide-react";
import DiscordIcon from "../components/DiscordIcon";
import { LINKS, openLink } from "../lib/api";

export default function Help() {
  const { t } = useTranslation();
  const faq = [1, 2, 3, 4];
  return (
    <div>
      <h1 className="page-title">{t("help.title")}</h1>
      <div className="help-grid">
        <div className="card">
          <div className="card-title mb-8">{t("help.discordTitle")}</div>
          <p className="text-sm text-dim mb-12" style={{ lineHeight: 1.5 }}>
            {t("help.discordLead")}
          </p>
          <button className="btn btn-discord btn-lg" onClick={() => openLink(LINKS.discord)}>
            <DiscordIcon size={18} /> {t("help.discordButton")}
          </button>
        </div>

        <div className="card">
          <div className="card-title mb-8">{t("help.docsTitle")}</div>
          <p className="text-sm text-dim mb-12" style={{ lineHeight: 1.5 }}>
            {t("help.docsLead")}
          </p>
          <div className="actions" style={{ marginTop: 0 }}>
            <button className="btn btn-primary" onClick={() => openLink(LINKS.docs)}>
              <BookOpen size={14} /> {t("help.docsButton")}
            </button>
            <button className="btn btn-secondary" onClick={() => openLink(LINKS.website)}>
              <Globe size={14} /> {t("help.websiteButton")}
            </button>
            <button className="btn btn-secondary" onClick={() => openLink(LINKS.steamStore)}>
              <ExternalLink size={14} /> {t("help.storeButton")}
            </button>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-title mb-12">{t("help.faqTitle")}</div>
        {faq.map((n) => (
          <div key={n}>
            <div className="faq-q">{t(`help.faq${n}q`)}</div>
            <div className="faq-a">{t(`help.faq${n}a`)}</div>
          </div>
        ))}
      </div>

      <div className="card">
        <div className="card-title mb-8">{t("help.aboutTitle")}</div>
        <p className="text-sm text-dim" style={{ lineHeight: 1.6 }}>
          {t("help.aboutBody")}
          <br />
          {t("help.credits")}
          <br />
          <span className="text-muted">{t("help.thirdParty")}</span>
          <br />
          <span className="text-muted">{t("help.copyright")}</span>
        </p>
        <div className="actions">
          <button className="btn btn-secondary btn-sm" onClick={() => openLink(LINKS.appLicense)}>
            <FileText size={13} /> {t("help.licenseButton")}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => openLink(LINKS.appNotices)}>
            <FileText size={13} /> {t("help.noticesButton")}
          </button>
        </div>
      </div>
    </div>
  );
}
