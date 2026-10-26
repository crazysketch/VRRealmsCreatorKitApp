import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { CalendarDays, Play, Trophy, Vote } from "lucide-react";
import { call, LINKS, openLink, type Live } from "../lib/api";

const when = (ts: number) =>
  new Date(ts * 1000).toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
const day = (ts: number) => new Date(ts * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });

/** VR Realms right now: players, the hub, the next events, open polls and the world contest. Asked again every minute. */
export default function LiveCard() {
  const { t } = useTranslation();
  const [live, setLive] = useState<Live | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let stop = false;
    const ask = () =>
      call<Live>("get_live")
        .then((l) => {
          if (!stop) {
            setLive(l);
            setFailed(false);
          }
        })
        .catch(() => !stop && setFailed(true));
    ask();
    const timer = window.setInterval(ask, 60_000);
    return () => {
      stop = true;
      window.clearInterval(timer);
    };
  }, []);

  const c = live?.contest ?? null;
  const contestLine = !c
    ? ""
    : c.phase === "upcoming"
    ? t("home.contestUpcoming", { from: day(c.buildFrom), until: day(c.buildUntil), voteUntil: day(c.voteUntil) })
    : c.phase === "building"
    ? t("home.contestBuilding", { until: day(c.buildUntil), count: c.entries })
    : c.phase === "voting"
    ? t("home.contestVoting", { voteUntil: day(c.voteUntil), count: c.entries })
    : t("home.contestClosed");

  return (
    <div className="card live-card">
      <div className="card-header">
        <span className="card-title">{t("home.liveTitle")}</span>
      </div>
      {failed && !live && <div className="text-sm text-muted">{t("home.liveOffline")}</div>}
      {!failed && !live && <div className="text-sm text-muted">{t("common.loading")}</div>}
      {live && (
        <>
          <div className="live-counts" title={t("home.liveOnlineHint")}>
            <div>
              <div className="live-num">{live.online ?? "–"}</div>
              <div className="live-lab">{t("home.liveOnline")}</div>
            </div>
            <div>
              <div className="live-num">{live.hub ?? "–"}</div>
              <div className="live-lab">{t("home.liveHub")}</div>
            </div>
            <button className="btn btn-accent" onClick={() => openLink(LINKS.playGame)} title={t("home.livePlayHint")}>
              <Play size={14} /> {t("home.livePlay")}
            </button>
          </div>

          {c && (
            <div className="live-section contest">
              <div className="live-head">
                <Trophy size={13} /> {c.title}
              </div>
              <div className="live-text">{contestLine}</div>
              {c.prizes.length > 0 && c.phase !== "closed" && (
                <div className="live-text">
                  {t("home.contestPrizes", { prizes: c.prizes.map((p) => p.toLocaleString()).join(" / "), entry: c.entryGold.toLocaleString() })}
                </div>
              )}
              {c.url && (
                <button className="news-link" onClick={() => openLink(c.url!)}>
                  {t("home.contestOpen")}
                </button>
              )}
            </div>
          )}

          <div className="live-section">
            <div className="live-head">
              <CalendarDays size={13} /> {t("home.liveEvents")}
            </div>
            {live.events.length === 0 && <div className="live-text">{t("home.liveNoEvents")}</div>}
            {live.events.slice(0, 3).map((e, i) => (
              <div className="live-row" key={i}>
                <span className="live-row-title">{e.title}</span>
                {e.live ? <span className="badge badge-success">{t("home.liveNow")}</span> : <span className="live-when">{when(e.startsAt)}</span>}
              </div>
            ))}
            <button className="news-link" onClick={() => openLink(LINKS.events)}>
              {t("home.liveAllEvents")}
            </button>
          </div>

          {live.polls.length > 0 && (
            <div className="live-section">
              <div className="live-head">
                <Vote size={13} /> {t("home.livePolls")}
              </div>
              {live.polls.slice(0, 3).map((p, i) => (
                <div className="live-row" key={i}>
                  <span className="live-row-title" title={p.question}>
                    {p.question}
                    <span className="live-sub">
                      {p.closesAt ? t("home.livePollCloses", { date: day(p.closesAt) }) : ""}
                      {p.votes != null ? ` · ${t("home.livePollVotes", { count: p.votes })}` : ""}
                    </span>
                  </span>
                  <button className="btn btn-secondary btn-sm" onClick={() => openLink(p.url ?? LINKS.polls)}>
                    {t("home.livePollVote")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
