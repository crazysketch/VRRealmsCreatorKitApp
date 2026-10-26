import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ExternalLink, Newspaper } from "lucide-react";
import { call, openLink, type NewsItem } from "../lib/api";

interface News {
  items: NewsItem[];
  source: string;
}

export default function NewsCard() {
  const { t } = useTranslation();
  const [news, setNews] = useState<News | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    call<News>("get_news")
      .then(setNews)
      .catch(() => setFailed(true));
  }, []);

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title" style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Newspaper size={13} /> {news?.source === "github" ? t("home.newsFromGithub") : t("home.news")}
        </span>
      </div>
      {failed && <div className="text-sm text-muted">{t("home.newsOffline")}</div>}
      {!failed && !news && <div className="text-sm text-muted">{t("common.loading")}</div>}
      {news && news.items.length === 0 && <div className="text-sm text-muted">{t("home.newsEmpty")}</div>}
      {news?.items.map((item, i) => (
        <div className="news-item" key={i}>
          <div className="news-meta">
            {item.tag && <span className="news-tag">{item.tag}</span>}
            {item.date && <span>{item.date}</span>}
          </div>
          <div className="news-title">{item.title}</div>
          {item.body && <div className="news-body">{item.body}</div>}
          {item.url && (
            <button className="news-link" onClick={() => openLink(item.url!)}>
              {t("home.readMore")} <ExternalLink size={11} />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
