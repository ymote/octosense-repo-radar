use crate::{functions::*, provider};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_PINS: usize = 100;
pub const MAX_SNAPSHOTS: usize = 96;
const MAX_REPOS: usize = 128;
const MAX_SEARCHES: usize = 8;
const STATE_LIMIT: usize = 4 * 1024 * 1024;
pub const STATE_FILE: &str = "repo-radar-v1.json";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cache<T> {
    pub data: T,
    pub observed: i64,
    pub checked: i64,
    pub etag: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct List<T> {
    pub items: Vec<T>,
    pub page: Page,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoCache {
    pub summary: Repo,
    pub touched: i64,
    pub overview: Option<Cache<Overview>>,
    pub pulls: Option<Cache<List<Pull>>>,
    pub releases: Option<Cache<List<Release>>>,
    pub snapshots: Vec<Snapshot>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedPin {
    pub repo_id: u64,
    pub pinned_at: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Rate {
    pub remaining: Option<u32>,
    pub reset: Option<i64>,
    pub cooldown: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    pub revision: u64,
    pub selected_repo_id: Option<u64>,
    pub pins: Vec<SavedPin>,
    pub repos: BTreeMap<u64, RepoCache>,
    pub searches: BTreeMap<String, Cache<List<Repo>>>,
    pub core_rate: Rate,
    pub search_rate: Rate,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            revision: 0,
            selected_repo_id: None,
            pins: vec![],
            repos: BTreeMap::new(),
            searches: BTreeMap::new(),
            core_rate: Rate::default(),
            search_rate: Rate::default(),
        }
    }
}
pub struct Store {
    pub state: State,
    root: PathBuf,
}
impl Store {
    // Native exports intentionally refuse filesystem access; tests use owned roots.
    #[cfg_attr(not(any(target_os = "wasi", test)), allow(dead_code))]
    pub fn load(root: &Path) -> Result<Self, String> {
        let file = root.join(STATE_FILE);
        let state = match std::fs::File::open(&file) {
            Ok(f) => {
                let mut b = Vec::new();
                f.take((STATE_LIMIT + 1) as u64)
                    .read_to_end(&mut b)
                    .map_err(|_| "Cannot read the private watchlist.")?;
                if b.len() > STATE_LIMIT {
                    return Err(
                        "Saved watchlist exceeds its size limit; it was not overwritten.".into(),
                    );
                }
                let state: State = serde_json::from_slice(&b).map_err(|_| {
                    "Saved watchlist is damaged; it was not overwritten.".to_string()
                })?;
                validate(&state)?;
                state
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(_) => return Err("Cannot open the app's private watchlist.".into()),
        };
        Ok(Self {
            state,
            root: root.to_path_buf(),
        })
    }
    pub fn save(&mut self) -> Result<(), String> {
        self.prune();
        validate(&self.state)?;
        let bytes = self.encoded_within_budget()?;
        std::fs::create_dir_all(&self.root).map_err(|_| "Cannot create private app storage.")?;
        let pending = self.root.join("repo-radar-v1.pending.json");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&pending)
            .map_err(|_| "Cannot write private app storage.")?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "Cannot finish saving the watchlist.")?;
        drop(file);
        std::fs::rename(&pending, self.root.join(STATE_FILE))
            .map_err(|_| "Cannot replace the watchlist; previous saved data remains.")?;
        Ok(())
    }
    /// Pins and their canonical summaries are durable. Endpoint responses and
    /// chart history are optional, including those belonging to pinned projects.
    /// Count limits alone cannot bound UTF-8 release notes and duplicate history.
    fn encoded_within_budget(&mut self) -> Result<Vec<u8>, String> {
        let bytes = encode(&self.state)?;
        if bytes.len() <= STATE_LIMIT {
            return Ok(bytes);
        }
        let mut remaining = bytes.len();
        drop(bytes);
        let mut candidates = Vec::new();
        for (key, cache) in &self.state.searches {
            candidates.push((cache.checked, Evict::Search(key.clone())));
        }
        for (id, cache) in &self.state.repos {
            if let Some(value) = &cache.pulls {
                candidates.push((value.checked, Evict::Pulls(*id)));
            }
            if let Some(value) = &cache.releases {
                candidates.push((value.checked, Evict::Releases(*id)));
            }
            if let Some(value) = &cache.overview {
                if !cache.snapshots.is_empty() || !value.data.snapshots.is_empty() {
                    candidates.push((value.observed, Evict::History(*id)));
                }
                candidates.push((value.checked, Evict::Overview(*id)));
            } else if !cache.snapshots.is_empty() {
                candidates.push((cache.touched, Evict::History(*id)));
            }
        }
        // Stable tie-breaking keeps eviction deterministic across restarts.
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, candidate) in candidates {
            if remaining <= STATE_LIMIT {
                break;
            }
            let freed = match candidate {
                Evict::Search(key) => {
                    // Ignoring the removed JSON key/comma slightly overestimates
                    // remaining bytes; it never permits an oversized final file.
                    self.state
                        .searches
                        .remove(&key)
                        .map(|c| encode(&c).map(|b| b.len()))
                        .transpose()?
                        .unwrap_or(0)
                }
                Evict::Pulls(id) => {
                    clear_cache(&mut self.state.repos.get_mut(&id).expect("known cache").pulls)?
                }
                Evict::Releases(id) => {
                    clear_cache(&mut self.state.repos.get_mut(&id).expect("known cache").releases)?
                }
                Evict::Overview(id) => {
                    clear_cache(&mut self.state.repos.get_mut(&id).expect("known cache").overview)?
                }
                Evict::History(id) => {
                    let cache = self.state.repos.get_mut(&id).expect("known cache");
                    let mut freed = encode(&cache.snapshots)?.len().saturating_sub(2);
                    cache.snapshots.clear();
                    if let Some(overview) = cache.overview.as_mut() {
                        freed += encode(&overview.data.snapshots)?.len().saturating_sub(2);
                        overview.data.snapshots.clear();
                    }
                    freed
                }
            };
            remaining = remaining.saturating_sub(freed);
        }
        let bytes = encode(&self.state)?;
        if bytes.len() > STATE_LIMIT {
            return Err("Saved project metadata exceeds the private storage limit; previous saved data remains.".into());
        }
        Ok(bytes)
    }

    pub fn bump(&mut self) {
        self.state.revision = self.state.revision.saturating_add(1);
    }
    pub fn remember(&mut self, repo: Repo, now: i64) {
        let id = repo.id;
        let cache = self.state.repos.entry(id).or_insert_with(|| RepoCache {
            summary: repo.clone(),
            touched: now,
            overview: None,
            pulls: None,
            releases: None,
            snapshots: vec![],
        });
        // Search totals do not supersede an overview observation. Canonical
        // names still follow the immutable ID when a repository is renamed.
        if cache.overview.is_none() {
            cache.summary = repo.clone();
        } else if let Some(overview) = cache.overview.as_mut().filter(|o| o.observed <= now) {
            cache.summary.full_name = repo.full_name.clone();
            cache.summary.html_url = repo.html_url.clone();
            overview.data.repo.full_name = repo.full_name;
            overview.data.repo.html_url = repo.html_url;
        }
        cache.touched = now;
    }
    pub fn id_for(&self, name: &str) -> Option<u64> {
        self.state
            .repos
            .iter()
            .find(|(_, c)| c.summary.full_name.eq_ignore_ascii_case(name))
            .map(|(id, _)| *id)
    }
    pub fn watchlist(&self) -> Watchlist {
        Watchlist {
            schema: 1,
            revision: self.state.revision,
            selected_repo_id: self.state.selected_repo_id,
            pins: self
                .state
                .pins
                .iter()
                .filter_map(|p| {
                    self.state.repos.get(&p.repo_id).map(|c| {
                        let o = c.overview.as_ref().map(|x| &x.data);
                        Pin {
                            repo: c.summary.clone(),
                            pinned_at: p.pinned_at.clone(),
                            last_observed_at: o.map(|o| o.observed_at.clone()),
                            star_delta: o.and_then(|o| o.star_delta),
                            fork_delta: o.and_then(|o| o.fork_delta),
                        }
                    })
                })
                .collect(),
        }
    }
    fn prune(&mut self) {
        let pinned: BTreeSet<_> = self.state.pins.iter().map(|p| p.repo_id).collect();
        let mut old: Vec<_> = self
            .state
            .repos
            .iter()
            .filter(|(id, _)| !pinned.contains(id))
            .map(|(id, c)| (*id, c.touched))
            .collect();
        old.sort_by_key(|(_, t)| *t);
        for (id, _) in old
            .into_iter()
            .take(self.state.repos.len().saturating_sub(MAX_REPOS))
        {
            self.state.repos.remove(&id);
        }
        let mut old: Vec<_> = self
            .state
            .searches
            .iter()
            .map(|(k, c)| (k.clone(), c.checked))
            .collect();
        old.sort_by_key(|(_, t)| *t);
        for (key, _) in old
            .into_iter()
            .take(self.state.searches.len().saturating_sub(MAX_SEARCHES))
        {
            self.state.searches.remove(&key);
        }
    }
}
fn validate(s: &State) -> Result<(), String> {
    let bad = || {
        "Saved watchlist has an unsupported or inconsistent format; it was not overwritten."
            .to_string()
    };
    if s.schema != 1
        || s.pins.len() > MAX_PINS
        || s.repos.len() > MAX_REPOS
        || s.searches.len() > MAX_SEARCHES
    {
        return Err(bad());
    }
    let mut ids = BTreeSet::new();
    for p in &s.pins {
        if !ids.insert(p.repo_id) || !s.repos.contains_key(&p.repo_id) {
            return Err(bad());
        }
    }
    if s.selected_repo_id.is_some_and(|id| !ids.contains(&id)) {
        return Err(bad());
    }
    for (id, c) in &s.repos {
        if *id == 0
            || *id != c.summary.id
            || provider::full_name(&c.summary.full_name).is_err()
            || c.snapshots.len() > MAX_SNAPSHOTS
            || c.overview
                .as_ref()
                .is_some_and(|o| o.data.repo.id != *id || o.data.snapshots.len() > MAX_SNAPSHOTS)
            || c.pulls.as_ref().is_some_and(|x| x.data.items.len() > 10)
            || c.releases.as_ref().is_some_and(|x| x.data.items.len() > 5)
        {
            return Err(bad());
        }
    }
    if s.searches.values().any(|x| x.data.items.len() > 10) {
        return Err(bad());
    }
    Ok(())
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
enum Evict {
    Search(String),
    Pulls(u64),
    Releases(u64),
    History(u64),
    Overview(u64),
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|_| "Cannot encode the private watchlist.".into())
}
fn clear_cache<T: Serialize>(value: &mut Option<Cache<T>>) -> Result<usize, String> {
    let before = encode(value)?.len();
    *value = None;
    // JSON null replaces the old value; field names remain in the record.
    Ok(before.saturating_sub(4))
}
