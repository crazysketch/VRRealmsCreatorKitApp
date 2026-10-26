import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { BookOpen, ExternalLink, Hammer, MessageSquare, Play, RefreshCw, ThumbsDown, ThumbsUp, Users } from "lucide-react";
import { call, LINKS, openLink, type Creation, type Creations as CreationsData, type InGame } from "../lib/api";
import type { Ctx } from "../lib/ctx";

type Filter = "all" | "world" | "avatar";
type Sort = "newest" | "played";

const STATE: Record<InGame, { label: string; hint?: string; badge: string } | null> = {
  approved: { label: "creations.stateApproved", badge: "badge-success" },
  not_verified: { label: "creations.stateNotVerified", hint: "creations.stateNotVerifiedHint", badge: "badge-info" },
  update_waiting: { label: "creations.stateUpdateWaiting", hint: "creations.stateUpdateWaitingHint", badge: "badge-warning" },
  waiting: { label: "creations.stateWaiting", hint: "creations.stateWaitingHint", badge: "badge-warning" },
  blocked: { label: "creations.stateBlocked", hint: "creations.stateBlockedHint", badge: "badge-danger" },
  unknown: null,
};

const sum = (items: Creation[], pick: (c: Creation) => number | null) => items.reduce((n, c) => n + (pick(c) ?? 0), 0);

export default function Creations({ ctx }: { ctx: Ctx }) {
  const { t } = useTranslation();
  const [data, setData] = useState<CreationsData | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("newest");

  const load = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      setData(await call<CreationsData>("get_creations"));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const items = data?.items ?? [];
  const worlds = items.filter((c) => c.kind === "world");
  const avatars = items.filter((c) => c.kind === "avatar");
  const shown = useMemo(() => {
    const list = items.filter((c) => (filter === "all" ? c.kind !== "other" : c.kind === filter));
    return sort === "newest" ? list : [...list].sort((a, b) => (b.players ?? 0) - (a.players ?? 0));
  }, [items, filter, sort]);

  const kitReady = ctx.overview?.engine.state === "ok" && !!ctx.overview?.kit?.exists;
  const openKit = () => (kitReady ? ctx.run("open", () => call("open_kit", { path: null })) : ctx.go("home"));

  const getBuilding = (text: string, what: Filter) => (
    <div className="card empty-card">
      <Hammer size={28} className="empty-icon" />
      <div className="empty-title">{t("creations.emptyTitle")}</div>
      <div className="empty-lead">{text}</div>
      <div className="actions" style={{ justifyContent: "center" }}>
        <button className="btn btn-accent" onClick={openKit} disabled={!!ctx.busy}>
          <Play size={14} /> {t("creations.emptyOpenKit")}
        </button>
        {what !== "avatar" && (
          <button className="btn btn-secondary" onClick={() => openLink(LINKS.buildWorld)}>
            <BookOpen size={14} /> {t("creations.emptyHowWorld")}
          </button>
        )}
        {what !== "world" && (
          <button className="btn btn-secondary" onClick={() => openLink(LINKS.buildAvatar)}>
            <BookOpen size={14} /> {t("creations.emptyHowAvatar")}
          </button>
        )}
      </div>
    </div>
  );

  const name = data?.name ?? t("creations.accountNoName");

  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title" style={{ marginBottom: 4 }}>
            {t("creations.title")}
          </h1>
          <div className="text-muted text-sm">{t("creations.lead")}</div>
          {data?.hasAccount && (
            <div className="text-muted text-sm" style={{ marginTop: 4 }}>
              {data.signedInNow ? t("creations.account", { name }) : t("creations.accountLast", { name })}
            </div>
          )}
        </div>
        <div className="actions">
          {data?.workshopUrl && (
            <button className="btn btn-secondary btn-sm" onClick={() => openLink(data.workshopUrl!)}>
              <ExternalLink size={13} /> {t("creations.workshopPage")}
            </button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={load} disabled={loading}>
            <RefreshCw size={13} className={loading ? "spin" : ""} /> {t("creations.refresh")}
          </button>
        </div>
      </div>

      {error && <div className="message error">{t("creations.offline", { error })}</div>}
      {!data && !error && <div className="text-muted">{t("common.loading")}</div>}

      {data && !data.hasAccount && (
        <div className="card empty-card">
          <Users size={28} className="empty-icon" />
          <div className="empty-title">{t("creations.noAccountTitle")}</div>
          <div className="empty-lead">{t("creations.noAccountLead")}</div>
          <div className="actions" style={{ justifyContent: "center" }}>
            <button className="btn btn-secondary" onClick={() => openLink("steam://open/main")}>
              {t("check.steamOpen")}
            </button>
          </div>
        </div>
      )}

      {data?.hasAccount && items.length === 0 && getBuilding(t("creations.emptyLead"), "all")}

      {data?.hasAccount && items.length > 0 && (
        <>
          <div className="stat-tiles">
            <Tile value={sum(worlds, (c) => c.playingNow)} label={t("creations.tileNow")} />
            <Tile value={sum(avatars, (c) => c.playingNow)} label={t("creations.tileWearingNow")} />
            <Tile value={sum(worlds, (c) => c.visits)} label={t("creations.tileVisits")} />
            <Tile value={sum(items, (c) => c.votesUp)} label={t("creations.tileThumbs")} />
          </div>

          <div className="filter-bar">
            <div className="seg">
              {(["all", "world", "avatar"] as Filter[]).map((f) => (
                <button key={f} className={filter === f ? "on" : ""} onClick={() => setFilter(f)}>
                  {f === "all" ? t("creations.all") : f === "world" ? t("creations.worlds") : t("creations.avatars")}
                  <span className="seg-count">{f === "all" ? worlds.length + avatars.length : f === "world" ? worlds.length : avatars.length}</span>
                </button>
              ))}
            </div>
            <div className="seg">
              {(["newest", "played"] as Sort[]).map((s) => (
                <button key={s} className={sort === s ? "on" : ""} onClick={() => setSort(s)}>
                  {s === "newest" ? t("creations.sortNewest") : t("creations.sortPlayed")}
                </button>
              ))}
            </div>
          </div>

          {shown.length === 0 && getBuilding(filter === "world" ? t("creations.emptyWorlds") : t("creations.emptyAvatars"), filter)}
          <div className="creation-grid">
            {shown.map((c) => (
              <CreationCard key={c.id} c={c} />
            ))}
          </div>
          <div className="text-muted text-sm" style={{ marginTop: 14, lineHeight: 1.5 }}>
            {t("creations.footnote")}
          </div>
        </>
      )}
    </div>
  );
}

function Tile({ value, label }: { value: number; label: string }) {
  return (
    <div className="stat-tile">
      <div className="stat-value">{value.toLocaleString()}</div>
      <div className="stat-label">{label}</div>
    </div>
  );
}

function CreationCard({ c }: { c: Creation }) {
  const { t } = useTranslation();
  const state = STATE[c.inGame];
  const world = c.kind === "world";
  const minutes = c.minutes ?? 0;
  const played =
    minutes >= 60
      ? t(world ? "creations.hours" : "creations.hoursWorn", { count: Math.round(minutes / 60) })
      : t(world ? "creations.minutes" : "creations.minutesWorn", { count: minutes });
  const date = c.updated ? new Date(c.updated * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" }) : "";

  return (
    <div className="creation-card">
      <button className="creation-pic" onClick={() => c.url && openLink(c.url)} title={t("creations.open")}>
        {c.preview ? <img src={c.preview} alt="" loading="lazy" /> : <div className="creation-pic-none" />}
        {(c.playingNow ?? 0) > 0 && (
          <span className="live-chip">
            <span className="live-dot" />
            {t(world ? "creations.nowIn" : "creations.nowWearing", { count: c.playingNow ?? 0 })}
          </span>
        )}
      </button>
      <div className="creation-body">
        <div className="creation-title" title={c.title}>
          {c.title || c.id}
        </div>
        <div className="creation-meta">
          {state && (
            <span className={`badge ${state.badge}`} title={state.hint ? t(state.hint) : undefined}>
              {t(state.label)}
            </span>
          )}
          {date && <span>{t("creations.updated", { date })}</span>}
        </div>
        {c.players != null && (
          <div className="creation-stats">
            <b>{world ? t("creations.worldPlayers", { count: c.players }) : t("creations.avatarPlayers", { count: c.players })}</b>
            {world && c.visits != null && <span>{t("creations.worldVisits", { count: c.visits })}</span>}
            {minutes > 0 && <span>{played}</span>}
            {(c.playersWeek ?? 0) > 0 && <span>{t("creations.thisWeek", { count: c.playersWeek ?? 0 })}</span>}
          </div>
        )}
        <div className="creation-steam">
          <span title="Steam">
            <ThumbsUp size={12} /> {c.votesUp}
          </span>
          <span>
            <ThumbsDown size={12} /> {c.votesDown}
          </span>
          <span>{t("creations.subscribers", { count: c.subscribers })}</span>
        </div>
        <div className="creation-actions">
          {c.url && (
            <button className="btn btn-secondary btn-sm" onClick={() => openLink(c.url!)}>
              <ExternalLink size={12} /> {t("creations.open")}
            </button>
          )}
          {c.commentsUrl && (
            <button className="btn btn-secondary btn-sm" onClick={() => openLink(c.commentsUrl!)}>
              <MessageSquare size={12} /> {t("creations.comments", { count: c.comments })}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
