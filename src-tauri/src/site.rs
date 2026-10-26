//! What vr-realms.com tells the app: VR Realms right now (players, the hub, events, polls, the world
//! contest) and a creator's own Workshop items with how players use them. One public feed, read only:
//! the site answers with counts and never names a player.

use crate::error::AppResult;
use crate::{net, steam};
use serde::{Deserialize, Serialize};

const FEED: &str = "https://vr-realms.com/api/kit_app.php";

/// Links the app shows a button for. The window's own permissions say the same.
const LINK_PREFIXES: &[&str] = &[
    "https://vr-realms.com/",
    "https://steamcommunity.com/sharedfiles/filedetails/",
    "https://steamcommunity.com/profiles/",
];

/// Where Workshop pictures come from (the window's image rule allows only these).
const PICTURE_PREFIXES: &[&str] = &["https://images.steamusercontent.com/", "https://steamuserimages-a.akamaihd.net/"];

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

fn safe_url(u: &str, prefixes: &[&str]) -> Option<String> {
    let u = u.trim();
    (u.len() <= 500 && !u.contains(char::is_whitespace) && prefixes.iter().any(|p| u.starts_with(p))).then(|| u.to_string())
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())
}

// ----- VR Realms right now ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveEvent {
    pub title: String,
    #[serde(default)]
    pub starts_at: i64,
    #[serde(default)]
    pub minutes: i64,
    #[serde(default)]
    pub world: String,
    #[serde(default)]
    pub live: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePoll {
    pub question: String,
    #[serde(default)]
    pub votes: Option<i64>,
    #[serde(default)]
    pub closes_at: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveContest {
    pub title: String,
    /// upcoming | building | voting | closed
    pub phase: String,
    #[serde(default)]
    pub build_from: i64,
    #[serde(default)]
    pub build_until: i64,
    #[serde(default)]
    pub vote_from: i64,
    #[serde(default)]
    pub vote_until: i64,
    #[serde(default)]
    pub prizes: Vec<i64>,
    #[serde(default)]
    pub entry_gold: i64,
    #[serde(default)]
    pub entries: i64,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    /// Players in worlds and the hub in the last two and a half minutes (a game at its own home is not counted).
    #[serde(default)]
    pub online: Option<i64>,
    /// Players on the hub servers; `None` when no hub reported.
    #[serde(default)]
    pub hub: Option<i64>,
    #[serde(default)]
    pub events: Vec<LiveEvent>,
    #[serde(default)]
    pub polls: Vec<LivePoll>,
    #[serde(default)]
    pub contest: Option<LiveContest>,
}

fn tidy_live(mut l: Live) -> Live {
    l.online = l.online.filter(|n| *n >= 0);
    l.hub = l.hub.filter(|n| *n >= 0);
    l.events = l
        .events
        .into_iter()
        .take(4)
        .map(|e| LiveEvent { title: clip(&e.title, 80), world: clip(&e.world, 60), ..e })
        .filter(|e| !e.title.is_empty())
        .collect();
    l.polls = l
        .polls
        .into_iter()
        .take(4)
        .map(|p| LivePoll {
            question: clip(&p.question, 160),
            url: p.url.and_then(|u| safe_url(&u, &["https://vr-realms.com/"])),
            ..p
        })
        .filter(|p| !p.question.is_empty())
        .collect();
    l.contest = l.contest.and_then(|c| {
        let phase = c.phase.to_ascii_lowercase();
        ["upcoming", "building", "voting", "closed"].contains(&phase.as_str()).then(|| LiveContest {
            title: clip(&c.title, 80),
            phase,
            prizes: c.prizes.into_iter().take(5).collect(),
            url: c.url.and_then(|u| safe_url(&u, &["https://vr-realms.com/"])),
            ..c
        })
    });
    l
}

pub async fn live() -> AppResult<Live> {
    let l: Live = net::get_json(&format!("{FEED}?view=live"), 12).await?;
    Ok(tidy_live(l))
}

// ----- a creator's own items -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Creation {
    pub id: String,
    /// world | avatar | other
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub votes_up: i64,
    #[serde(default)]
    pub votes_down: i64,
    #[serde(default)]
    pub comments: i64,
    #[serde(default)]
    pub subscribers: i64,
    #[serde(default)]
    pub favorites: i64,
    /// approved | update_waiting | not_verified | waiting | blocked | unknown
    #[serde(default)]
    pub in_game: String,
    /// Players who visited the world, or wore the avatar, in the last 90 days (the creator not counted).
    #[serde(default)]
    pub players: Option<i64>,
    #[serde(default)]
    pub players_week: Option<i64>,
    #[serde(default)]
    pub playing_now: Option<i64>,
    /// Visits to a world (a world only).
    #[serde(default)]
    pub visits: Option<i64>,
    #[serde(default)]
    pub minutes: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub comments_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatorFeed {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    items: Vec<Creation>,
    #[serde(default)]
    workshop_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creations {
    /// False when no Steam account could be found on this PC.
    pub has_account: bool,
    pub signed_in_now: bool,
    pub name: Option<String>,
    pub items: Vec<Creation>,
    pub workshop_url: Option<String>,
}

fn tidy_creation(c: Creation) -> Option<Creation> {
    if !digits(&c.id) {
        return None;
    }
    let kind = match c.kind.as_str() {
        "world" | "avatar" => c.kind.clone(),
        _ => "other".to_string(),
    };
    let in_game = match c.in_game.as_str() {
        "approved" | "update_waiting" | "not_verified" | "waiting" | "blocked" => c.in_game.clone(),
        _ => "unknown".to_string(),
    };
    let non_neg = |v: Option<i64>| v.filter(|n| *n >= 0);
    Some(Creation {
        title: clip(&c.title, 100),
        preview: safe_url(&c.preview, PICTURE_PREFIXES).unwrap_or_default(),
        tags: c.tags.iter().take(8).map(|t| clip(t, 24)).filter(|t| !t.is_empty()).collect(),
        url: c.url.as_deref().and_then(|u| safe_url(u, LINK_PREFIXES)),
        comments_url: c.comments_url.as_deref().and_then(|u| safe_url(u, LINK_PREFIXES)),
        players: non_neg(c.players),
        players_week: non_neg(c.players_week),
        playing_now: non_neg(c.playing_now),
        visits: non_neg(c.visits),
        minutes: non_neg(c.minutes),
        votes_up: c.votes_up.max(0),
        votes_down: c.votes_down.max(0),
        comments: c.comments.max(0),
        subscribers: c.subscribers.max(0),
        favorites: c.favorites.max(0),
        kind,
        in_game,
        ..c
    })
}

pub async fn creations() -> AppResult<Creations> {
    let Some(account) = steam::this_pc_account() else {
        return Ok(Creations { has_account: false, signed_in_now: false, name: None, items: Vec::new(), workshop_url: None });
    };
    let feed: CreatorFeed = net::get_json(&format!("{FEED}?view=creator&steam_id={}", account.steam_id), 20).await?;
    Ok(Creations {
        has_account: true,
        signed_in_now: account.signed_in_now,
        name: account.persona.or(feed.name).map(|n| clip(&n, 40)),
        items: feed.items.into_iter().filter_map(tidy_creation).take(300).collect(),
        workshop_url: feed.workshop_url.as_deref().and_then(|u| safe_url(u, LINK_PREFIXES)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creation(json: &str) -> Option<Creation> {
        tidy_creation(serde_json::from_str(json).unwrap())
    }

    #[test]
    fn creations_keep_only_our_links_and_steams_pictures() {
        let c = creation(
            r#"{"id":"3756607915","kind":"world","title":"Jog at the Park","preview":"https://images.steamusercontent.com/ugc/1/2/",
                "url":"https://steamcommunity.com/sharedfiles/filedetails/?id=3756607915","commentsUrl":"https://evil.example/x",
                "inGame":"approved","players":79,"visits":220,"votesUp":2}"#,
        )
        .unwrap();
        assert_eq!(c.preview, "https://images.steamusercontent.com/ugc/1/2/");
        assert!(c.url.is_some());
        assert!(c.comments_url.is_none());
        assert_eq!(c.players, Some(79));
        let bad = creation(r#"{"id":"1","kind":"world","preview":"https://tracker.example/p.png","inGame":"hacked"}"#).unwrap();
        assert_eq!(bad.preview, "");
        assert_eq!(bad.in_game, "unknown");
        assert!(creation(r#"{"id":"12a","kind":"world"}"#).is_none());
        assert_eq!(creation(r#"{"id":"5","kind":"script"}"#).unwrap().kind, "other");
    }

    #[test]
    #[ignore = "asks the real vr-realms.com and reads this PC's Steam account"]
    fn the_real_feed_answers() {
        let live = tauri::async_runtime::block_on(live()).unwrap();
        println!("live: online {:?}, hub {:?}, {} events, {} polls, contest {:?}", live.online, live.hub, live.events.len(),
            live.polls.len(), live.contest.as_ref().map(|c| c.phase.clone()));
        let mine = tauri::async_runtime::block_on(creations()).unwrap();
        println!("creations: account {}, signed in {}, {:?}, {} items", mine.has_account, mine.signed_in_now, mine.name, mine.items.len());
        for c in mine.items.iter().take(3) {
            println!("  {} {} {:?} players {:?} in game {}", c.kind, c.title, c.preview.get(..40), c.players, c.in_game);
        }
        assert!(mine.items.iter().all(|c| c.url.is_some() && c.comments_url.is_some()));
    }

    #[test]
    fn the_sites_real_answers_are_read() {
        let live: Live = serde_json::from_str(
            r#"{"ok":true,"view":"live","online":3,"hub":null,"events":[{"title":"Saturday Hangout","startsAt":1793458800,"minutes":120,
                "world":"THE HUB","live":false}],"polls":[{"question":"Q?","votes":null,"closesAt":1793606340,
                "url":"https://vr-realms.com/polls.php#poll-9"},{"question":"Bad","url":"javascript:alert(1)"}],
                "contest":{"title":"Thanksgiving World Contest","phase":"upcoming","buildFrom":1,"buildUntil":2,"voteFrom":3,
                "voteUntil":4,"prizes":[3000,2000,1000],"entryGold":250,"entries":0,"url":"https://vr-realms.com/contest"},
                "playUrl":"steam://run/2964550"}"#,
        )
        .unwrap();
        let live = tidy_live(live);
        assert_eq!(live.online, Some(3));
        assert_eq!(live.hub, None);
        assert_eq!(live.events.len(), 1);
        assert!(live.polls[0].url.is_some());
        assert!(live.polls[1].url.is_none());
        assert_eq!(live.contest.unwrap().prizes, vec![3000, 2000, 1000]);
        let odd: Live = serde_json::from_str(r#"{"contest":{"title":"X","phase":"<b>"}}"#).unwrap();
        assert!(tidy_live(odd).contest.is_none());
    }
}
