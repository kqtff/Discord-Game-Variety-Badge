//! Discord's list of auto-detectable games, and picking which ones to run.

use rand::seq::SliceRandom;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

const DISCORD_URL: &str = "https://discord.com/api/applications/detectable";
const MIRROR_URL: &str = "https://markterence.github.io/discord-quest-completer/detectable.json";
const CACHE_FILE: &str = "gamelist.cache.json";

/// Trimmed copy of the list (Windows executables only), used when offline.
const BUNDLED: &[u8] = include_bytes!("../assets/gamelist.min.json");
/// Well-known games, shown first. Names match Discord's list exactly.
const POPULAR: &str = include_str!("../assets/popular.json");

#[derive(Deserialize, Clone)]
struct RawExe {
    name: String,
    #[serde(default)]
    is_launcher: bool,
    #[serde(default)]
    os: Option<String>,
}

#[derive(Deserialize, Clone)]
struct RawGame {
    id: String,
    name: String,
    #[serde(default)]
    executables: Option<Vec<RawExe>>,
}

/// A game that can be faked: its Discord app ID and the relative path of the
/// executable Discord looks for (e.g. `_retail_/Wow.exe`).
#[derive(Clone, Debug)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub exe_path: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Discord,
    Mirror,
    Cache,
    Bundled,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Discord => "Discord (latest)",
            Source::Mirror => "GitHub mirror",
            Source::Cache => "saved copy",
            Source::Bundled => "built-in copy",
        }
    }
}

/// Loads the newest list available: Discord, then the mirror, then the copy
/// saved last time, then the copy built into the app.
pub fn load() -> (Vec<Game>, Source) {
    for (url, source) in [(DISCORD_URL, Source::Discord), (MIRROR_URL, Source::Mirror)] {
        if let Some(text) = download(url) {
            if let Some(games) = parse(text.as_bytes()) {
                let _ = std::fs::write(cache_path(), &text);
                return (games, source);
            }
        }
    }
    if let Some(games) = std::fs::read(cache_path()).ok().and_then(|b| parse(&b)) {
        return (games, Source::Cache);
    }
    (parse(BUNDLED).unwrap_or_default(), Source::Bundled)
}

fn download(url: &str) -> Option<String> {
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(30))
        .set("User-Agent", "discord-game-variety-badge")
        .call()
        .ok()?;
    let mut text = String::new();
    resp.into_reader()
        .take(128 * 1024 * 1024)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

fn cache_path() -> PathBuf {
    crate::launcher::base_dir().join(CACHE_FILE)
}

fn parse(bytes: &[u8]) -> Option<Vec<Game>> {
    let raw: Vec<RawGame> = serde_json::from_slice(bytes).ok()?;
    let mut seen_paths = HashSet::new();
    let games: Vec<Game> = raw
        .into_iter()
        .filter_map(usable)
        // Two games with the same exe path would collide on disk.
        .filter(|g| seen_paths.insert(g.exe_path.to_string_lossy().to_lowercase()))
        .collect();
    (games.len() > 100).then_some(games)
}

fn usable(g: RawGame) -> Option<Game> {
    let exe = g.executables?.into_iter().find(|e| {
        e.os.as_deref() == Some("win32")
            && !e.is_launcher
            && !e.name.contains(['>', '<', ':', '"', '|', '?', '*'])
            && e.name.to_lowercase().ends_with(".exe")
    })?;
    let parts: Vec<&str> = exe
        .name
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != "..")
        .collect();
    if parts.is_empty() {
        return None;
    }
    Some(Game {
        id: g.id,
        name: g.name,
        exe_path: parts.iter().collect(),
    })
}

/// Popular games first (in the curated order), then everything else shuffled.
pub fn order(games: &[Game]) -> Vec<Game> {
    let popular: Vec<String> = serde_json::from_str(POPULAR).unwrap_or_default();
    let by_name: HashMap<&str, &Game> = games.iter().map(|g| (g.name.as_str(), g)).collect();

    let mut ordered: Vec<Game> = Vec::with_capacity(games.len());
    let mut used = HashSet::new();
    for name in &popular {
        if let Some(g) = by_name.get(name.as_str()) {
            if used.insert(g.id.clone()) {
                ordered.push((*g).clone());
            }
        }
    }
    let mut rest: Vec<Game> = games.iter().filter(|g| !used.contains(&g.id)).cloned().collect();
    rest.shuffle(&mut rand::thread_rng());
    ordered.extend(rest);
    ordered
}

/// Reshuffles only the non-popular part, keeping popular games on top.
pub fn reshuffle(ordered: &mut [Game]) {
    let popular: HashSet<String> = serde_json::from_str::<Vec<String>>(POPULAR)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let first_other = ordered
        .iter()
        .position(|g| !popular.contains(&g.name))
        .unwrap_or(ordered.len());
    ordered[first_other..].shuffle(&mut rand::thread_rng());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_list_parses() {
        let games = parse(BUNDLED).unwrap();
        assert!(games.len() > 5000);
        let ordered = order(&games);
        assert_eq!(ordered[0].name, "Overwatch");
        assert_eq!(ordered.len(), games.len());
    }

    #[test]
    fn exe_path_is_split_into_folders() {
        let g = usable(RawGame {
            id: "1".into(),
            name: "X".into(),
            executables: Some(vec![RawExe {
                name: "_retail_/wow.exe".into(),
                is_launcher: false,
                os: Some("win32".into()),
            }]),
        })
        .unwrap();
        assert_eq!(g.exe_path, PathBuf::from("_retail_").join("wow.exe"));
    }

    #[test]
    fn skips_unsafe_paths() {
        let g = usable(RawGame {
            id: "1".into(),
            name: "X".into(),
            executables: Some(vec![RawExe {
                name: "../../evil.exe".into(),
                is_launcher: false,
                os: Some("win32".into()),
            }]),
        })
        .unwrap();
        assert_eq!(g.exe_path, PathBuf::from("evil.exe"));
    }
}
