//! The News section. It reads a small public feed on vr-realms.com, so news can change without an
//! app update. When the feed cannot be reached, the newest Creator Kit release notes stand in.

use crate::error::AppResult;
use crate::net;
use serde::{Deserialize, Serialize};

const FEED_URL: &str = "https://vr-realms.com/api/kit_news.php";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub body: String,
    /// Only links on vr-realms.com, Discord, GitHub or the stores open (see the window's permissions).
    #[serde(default)]
    pub url: Option<String>,
    /// A short label such as "Event", "Update" or "Kit".
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Deserialize)]
struct Feed {
    items: Vec<NewsItem>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct News {
    pub items: Vec<NewsItem>,
    /// "website" or "github"
    pub source: String,
}

/// One line of release notes as plain text: no bold marks, code marks or link targets, and a bullet
/// for a list item.
fn plain(line: &str) -> String {
    let mut l = line.trim();
    let bullet = l.starts_with("- ") || l.starts_with("* ");
    if bullet {
        l = l[2..].trim_start();
    }
    let mut out = String::with_capacity(l.len());
    let mut rest = l;
    // [text](url) -> text
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|i| open + i) else { break };
        let Some(end) = rest[close..].find(')').map(|i| close + i) else { break };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    let out = out.replace("**", "").replace("__", "").replace('`', "");
    if bullet {
        format!("• {out}")
    } else {
        out
    }
}

/// The start of a release's notes, as plain text and short enough for the card.
fn summary(body: &str) -> String {
    let mut text = String::new();
    for line in body.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if text.chars().count() >= 220 {
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&plain(line));
    }
    clip(&text, 300)
}

/// Where a news link may lead. The window's own permissions say the same; this keeps a link the
/// window would refuse from showing a button at all.
const LINK_PREFIXES: &[&str] = &[
    "https://vr-realms.com/",
    "https://www.vr-realms.com/",
    "https://discord.com/invite/",
    "https://github.com/crazysketch/",
    "https://store.steampowered.com/",
    "https://store.epicgames.com/",
];

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Keeps a feed item short and its link to the places above.
fn tidy(item: NewsItem) -> Option<NewsItem> {
    let title = clip(&item.title, 120);
    if title.is_empty() {
        return None;
    }
    let url = item
        .url
        .map(|u| u.trim().to_string())
        .filter(|u| u.len() <= 500 && !u.contains(char::is_whitespace) && LINK_PREFIXES.iter().any(|p| u.starts_with(p)));
    Some(NewsItem {
        title,
        date: clip(&item.date, 20),
        body: clip(&item.body, 600),
        url,
        tag: item.tag.map(|t| clip(&t, 16)).filter(|t| !t.is_empty()),
    })
}

pub async fn fetch() -> AppResult<News> {
    if let Ok(feed) = net::get_json::<Feed>(FEED_URL, 10).await {
        let items: Vec<NewsItem> = feed.items.into_iter().filter_map(tidy).take(10).collect();
        if !items.is_empty() {
            return Ok(News { items, source: "website".into() });
        }
    }
    let url = format!("https://api.github.com/repos/{}/releases?per_page=3", crate::kit::REPO);
    let releases: Vec<GhRelease> = net::get_json(&url, 15).await?;
    let items = releases
        .into_iter()
        .map(|r| NewsItem {
            title: r.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Creator Kit {}", r.tag_name)),
            date: r.published_at.unwrap_or_default().chars().take(10).collect(),
            body: summary(r.body.as_deref().unwrap_or("")),
            url: Some(r.html_url),
            tag: Some("Kit".into()),
        })
        .filter_map(tidy)
        .collect();
    Ok(News { items, source: "github".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(url: &str) -> NewsItem {
        NewsItem { title: "Hi".into(), date: String::new(), body: String::new(), url: Some(url.into()), tag: None }
    }

    #[test]
    fn links_outside_our_places_are_dropped() {
        assert!(tidy(item("https://vr-realms.com/updates.php")).unwrap().url.is_some());
        assert!(tidy(item("https://github.com/crazysketch/VRRealmsCreatorKit/releases")).unwrap().url.is_some());
        assert!(tidy(item("https://vr-realms.com.evil.example/")).unwrap().url.is_none());
        assert!(tidy(item("http://vr-realms.com/")).unwrap().url.is_none());
        assert!(tidy(item("file:///C:/Windows/System32/calc.exe")).unwrap().url.is_none());
        assert!(tidy(item("https://github.com/someoneelse/x")).unwrap().url.is_none());
    }

    #[test]
    fn release_notes_become_plain_text() {
        assert_eq!(plain("- **SMG Pad** (Weapons): see [the docs](https://x.y/z)."), "• SMG Pad (Weapons): see the docs.");
        assert_eq!(plain("Use `vrr.Kit.ImportAvatar`"), "Use vrr.Kit.ImportAvatar");
        let s = summary("## New\nFirst line.\n- one\n- two\n\n### More\n- three");
        assert_eq!(s, "First line.\n• one\n• two\n• three");
    }

    #[test]
    fn long_text_is_clipped_and_empty_titles_dropped() {
        let mut i = item("https://vr-realms.com/");
        i.body = "x".repeat(5000);
        assert_eq!(tidy(i).unwrap().body.chars().count(), 601);
        let mut e = item("https://vr-realms.com/");
        e.title = "   ".into();
        assert!(tidy(e).is_none());
    }
}
