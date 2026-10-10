use crate::{
    functions::*,
    provider::{self, Github, Transport, Wire},
    storage::{Cache, List, MAX_PINS, MAX_SNAPSHOTS, Rate, SavedPin, Store},
};
use std::path::Path;

pub struct App<T: Transport> {
    pub store: Store,
    transport: T,
    now: i64,
}

pub fn with_app<R>(f: impl FnOnce(&mut App<Github>) -> Result<R, String>) -> Result<R, String> {
    #[cfg(target_os = "wasi")]
    {
        // Every component instance gets this app's private preopened directory.
        // The host serializes component invocations; no UI owns a second writer.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "The system clock is invalid.")?
            .as_secs() as i64;
        f(&mut App::new(Path::new("/"), Github, now)?)
    }
    #[cfg(not(target_os = "wasi"))]
    {
        let _ = f;
        Err("Run Repo Radar inside its OctoSense component host.".into())
    }
}

impl<T: Transport> App<T> {
    // Native exports intentionally refuse filesystem access; tests use owned roots.
    #[cfg_attr(not(any(target_os = "wasi", test)), allow(dead_code))]
    pub fn new(root: &Path, transport: T, now: i64) -> Result<Self, String> {
        Ok(Self {
            store: Store::load(root)?,
            transport,
            now,
        })
    }
    pub fn watchlist(&self) -> Watchlist {
        self.store.watchlist()
    }
    pub fn pin(&mut self, name: &str) -> Result<Watchlist, String> {
        let name = provider::full_name(name)?;
        let id = if let Some(id) = self.store.id_for(&name) {
            id
        } else {
            // A displayed saved search may use a pre-rename name or its
            // unpinned summary may have been evicted. Only verified cached
            // provider records can restore it; caller-supplied data cannot.
            let repo = self
                .store
                .state
                .searches
                .values()
                .filter_map(|cache| {
                    cache
                        .data
                        .items
                        .iter()
                        .find(|repo| repo.full_name.eq_ignore_ascii_case(&name))
                        .map(|repo| (cache.observed, repo.clone()))
                })
                .max_by_key(|(observed, _)| *observed)
                .map(|(_, repo)| repo)
                .ok_or("Search for this public project or load its overview before pinning it.")?;
            let id = repo.id;
            if !self.store.state.repos.contains_key(&id) {
                self.store.remember(repo, self.now);
            }
            id
        };
        if !self.store.state.pins.iter().any(|p| p.repo_id == id) {
            if self.store.state.pins.len() >= MAX_PINS {
                return Err(
                    "Your watchlist holds up to 100 projects. Unpin one before adding another."
                        .into(),
                );
            }
            self.store.state.pins.push(SavedPin {
                repo_id: id,
                pinned_at: provider::timestamp(self.now),
            });
        }
        self.store.state.selected_repo_id = Some(id);
        self.save_watchlist()
    }
    pub fn unpin(&mut self, id: u64) -> Result<Watchlist, String> {
        let index = self.pin_index(id)?;
        self.store.state.pins.remove(index);
        if self.store.state.selected_repo_id == Some(id) {
            let next = index.min(self.store.state.pins.len().saturating_sub(1));
            self.store.state.selected_repo_id = self.store.state.pins.get(next).map(|p| p.repo_id);
        }
        self.save_watchlist()
    }
    pub fn select(&mut self, id: u64) -> Result<Watchlist, String> {
        self.pin_index(id)?;
        self.store.state.selected_repo_id = Some(id);
        self.save_watchlist()
    }
    pub fn move_pin(&mut self, id: u64, direction: i32) -> Result<Watchlist, String> {
        if direction != -1 && direction != 1 {
            return Err("Move direction must be -1 (up) or 1 (down).".into());
        }
        let index = self.pin_index(id)?;
        let next = index as i32 + direction;
        if next < 0 || next as usize >= self.store.state.pins.len() {
            return Ok(self.watchlist());
        }
        self.store.state.pins.swap(index, next as usize);
        self.save_watchlist()
    }
    fn pin_index(&self, id: u64) -> Result<usize, String> {
        self.store
            .state
            .pins
            .iter()
            .position(|p| p.repo_id == id)
            .ok_or_else(|| "That project is not pinned in this watchlist.".into())
    }
    fn save_watchlist(&mut self) -> Result<Watchlist, String> {
        self.store.bump();
        self.store.save()?;
        Ok(self.watchlist())
    }
    pub fn search(&mut self, query: &str, page_number: u32) -> Result<SearchResponse, String> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 128 || query.chars().any(char::is_control) {
            return Err(
                "Enter a project search of 1–128 characters without control characters.".into(),
            );
        }
        if !(1..=100).contains(&page_number) {
            return Err(
                "Search pages must be between 1 and 100 (GitHub's 1,000-result limit).".into(),
            );
        }
        let key = format!("{query}\n{page_number}");
        let url = format!(
            "{}/search/repositories?q={}&per_page=10&page={page_number}",
            provider::API,
            provider::encode(query)
        );
        let old = self.store.state.searches.get(&key).cloned();
        let (meta, result) = fetch(
            &self.transport,
            self.now,
            &url,
            old,
            300,
            false,
            &mut self.store.state.search_rate,
            |wire| {
                let v = wire.json()?;
                let items = v["items"]
                    .as_array()
                    .ok_or("GitHub returned an invalid search list.")?
                    .iter()
                    .take(10)
                    .map(provider::repo)
                    .collect::<Result<Vec<_>, _>>()?;
                let total = provider::count(&v, "total_count")?;
                let incomplete = v["incomplete_results"]
                    .as_bool()
                    .ok_or("GitHub did not report search completeness.")?;
                let next =
                    (provider::has_next(wire) && page_number < 100).then_some(page_number + 1);
                let page = provider::page(
                    items.len(),
                    next,
                    Some(total),
                    incomplete,
                    &format!(
                        "Search page {page_number}; up to 10 of {total} matches. GitHub exposes at most 1,000 results."
                    ),
                );
                Ok(List { items, page })
            },
        );
        let data = if let Some(cache) = result {
            // A saved search is also verified provider data, even if its summary
            // was evicted independently from the bounded project cache.
            for repo in &cache.data.items {
                self.store.remember(repo.clone(), cache.observed);
                self.store
                    .state
                    .repos
                    .get_mut(&repo.id)
                    .expect("remembered repository")
                    .touched = self.now;
            }
            let data = cache.data.clone();
            self.store.state.searches.insert(key, cache);
            data
        } else {
            List {
                items: vec![],
                page: provider::page(0, None, None, false, "No search response is available."),
            }
        };
        self.store.save()?;
        Ok(SearchResponse {
            meta,
            page: data.page,
            items: data.items,
        })
    }
    pub fn overview(&mut self, name: &str, force: bool) -> Result<OverviewResponse, String> {
        let name = provider::full_name(name)?;
        let old = self
            .store
            .id_for(&name)
            .and_then(|id| self.store.state.repos[&id].overview.clone());
        let url = format!("{}/repos/{name}", provider::API);
        let (meta, mut result) = fetch(
            &self.transport,
            self.now,
            &url,
            old,
            900,
            force,
            &mut self.store.state.core_rate,
            |wire| provider::overview(&wire.json()?, self.now),
        );
        if let Some(cache) = result.as_mut() {
            let id = cache.data.repo.id;
            if meta.state == FetchState::Fresh {
                let previous = self
                    .store
                    .state
                    .repos
                    .get(&id)
                    .and_then(|c| c.overview.as_ref());
                let mut snapshots = self
                    .store
                    .state
                    .repos
                    .get(&id)
                    .map(|c| c.snapshots.clone())
                    .unwrap_or_default();
                if let Some(previous) = previous.filter(|p| p.observed < self.now) {
                    cache.data.previous_observed_at = Some(previous.data.observed_at.clone());
                    cache.data.star_delta =
                        Some(delta(cache.data.repo.stars, previous.data.repo.stars));
                    cache.data.fork_delta =
                        Some(delta(cache.data.repo.forks, previous.data.repo.forks));
                }
                if snapshots
                    .last()
                    .is_some_and(|s| s.observed_at == cache.data.observed_at)
                {
                    snapshots.pop();
                }
                snapshots.push(Snapshot {
                    observed_at: cache.data.observed_at.clone(),
                    stars: cache.data.repo.stars,
                    forks: cache.data.repo.forks,
                    open_issues_and_prs: cache.data.repo.open_issues_and_prs,
                });
                if snapshots.len() > MAX_SNAPSHOTS {
                    snapshots.drain(..snapshots.len() - MAX_SNAPSHOTS);
                }
                cache.data.snapshots = snapshots;
            }
            self.store.remember(cache.data.repo.clone(), self.now);
            let stored = self
                .store
                .state
                .repos
                .get_mut(&id)
                .expect("remembered repository");
            stored.summary = cache.data.repo.clone();
            stored.snapshots = cache.data.snapshots.clone();
            stored.overview = Some(cache.clone());
            if meta.state == FetchState::Fresh {
                self.store.bump();
            }
        }
        self.store.save()?;
        Ok(OverviewResponse {
            meta,
            value: result.map(|c| c.data),
        })
    }
    fn known(&self, name: &str) -> Result<(u64, String), String> {
        let name = provider::full_name(name)?;
        let id = self
            .store
            .id_for(&name)
            .ok_or("Search for this public project or load its overview first.")?;
        Ok((id, self.store.state.repos[&id].summary.full_name.clone()))
    }
    pub fn pulls(&mut self, name: &str, force: bool) -> Result<PullResponse, String> {
        let (id, name) = self.known(name)?;
        let url = format!(
            "{}/repos/{name}/pulls?state=all&sort=updated&direction=desc&per_page=10&page=1",
            provider::API
        );
        let old = self.store.state.repos[&id].pulls.clone();
        let (meta, result) = fetch(
            &self.transport,
            self.now,
            &url,
            old,
            300,
            force,
            &mut self.store.state.core_rate,
            |wire| {
                let items = provider::pulls(&wire.json()?, &name)?;
                let page = provider::page(
                    items.len(),
                    provider::has_next(wire).then_some(2),
                    None,
                    false,
                    "First 10 recently updated PRs at collection time; a bounded sample, not complete history or daily totals.",
                );
                Ok(List { items, page })
            },
        );
        let data = result
            .as_ref()
            .map(|c| c.data.clone())
            .unwrap_or_else(|| List {
                items: vec![],
                page: provider::page(0, None, None, false, "No PR response is available."),
            });
        let stored = self.store.state.repos.get_mut(&id).unwrap();
        stored.pulls = result;
        stored.touched = self.now;
        self.store.save()?;
        Ok(PullResponse {
            meta,
            page: data.page,
            items: data.items,
        })
    }
    pub fn releases(&mut self, name: &str, force: bool) -> Result<ReleaseResponse, String> {
        let (id, name) = self.known(name)?;
        let url = format!("{}/repos/{name}/releases?per_page=5&page=1", provider::API);
        let old = self.store.state.repos[&id].releases.clone();
        let (meta, result) = fetch(
            &self.transport,
            self.now,
            &url,
            old,
            300,
            force,
            &mut self.store.state.core_rate,
            |wire| {
                let items = provider::releases(&wire.json()?, &name)?;
                let page = provider::page(
                    items.len(),
                    provider::has_next(wire).then_some(2),
                    None,
                    false,
                    "First 5 published releases returned by GitHub; older releases may exist. Notes are limited to 2,048 characters.",
                );
                Ok(List { items, page })
            },
        );
        let data = result
            .as_ref()
            .map(|c| c.data.clone())
            .unwrap_or_else(|| List {
                items: vec![],
                page: provider::page(0, None, None, false, "No release response is available."),
            });
        let stored = self.store.state.repos.get_mut(&id).unwrap();
        stored.releases = result;
        stored.touched = self.now;
        self.store.save()?;
        Ok(ReleaseResponse {
            meta,
            page: data.page,
            items: data.items,
        })
    }
}
fn delta(now: u64, before: u64) -> i64 {
    (i128::from(now) - i128::from(before)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn meta<C>(
    state: FetchState,
    message: &str,
    cache: &Option<Cache<C>>,
    rate: &Rate,
    url: &str,
    now: i64,
) -> Meta {
    Meta {
        cached: cache.is_some() && state != FetchState::Fresh,
        state,
        message: message.into(),
        fetched_at: cache
            .as_ref()
            .map(|c| provider::timestamp(c.observed))
            .unwrap_or_default(),
        rate_remaining: rate.remaining,
        retry_at: (rate.cooldown > now).then(|| provider::timestamp(rate.cooldown)),
        source_url: url.into(),
    }
}
#[allow(clippy::too_many_arguments)]
fn fetch<C: Clone>(
    transport: &impl Transport,
    now: i64,
    url: &str,
    mut old: Option<Cache<C>>,
    ttl: i64,
    force: bool,
    rate: &mut Rate,
    parse: impl FnOnce(&Wire) -> Result<C, String>,
) -> (Meta, Option<Cache<C>>) {
    if !force
        && old
            .as_ref()
            .is_some_and(|c| now >= c.checked && now - c.checked < ttl)
    {
        return (
            meta(
                FetchState::Cached,
                "Saved response is within its cache lifetime.",
                &old,
                rate,
                url,
                now,
            ),
            old,
        );
    }
    if rate.remaining == Some(0) && rate.reset.is_some_and(|reset| reset > now) {
        rate.cooldown = rate.cooldown.max(rate.reset.unwrap());
    }
    if rate.cooldown > now {
        return (
            meta(
                FetchState::RateLimited,
                "GitHub's request budget is cooling down. Retry after the indicated time.",
                &old,
                rate,
                url,
                now,
            ),
            old,
        );
    }
    let response = match transport.get(url, old.as_ref().and_then(|c| c.etag.as_deref())) {
        Ok(w) => w,
        Err(message) => {
            return (
                meta(FetchState::Failed, &message, &old, rate, url, now),
                old,
            );
        }
    };
    if let Some(n) = response
        .header("x-ratelimit-remaining")
        .and_then(|s| s.parse().ok())
    {
        rate.remaining = Some(n);
    }
    if let Some(n) = response
        .header("x-ratelimit-reset")
        .and_then(|s| s.parse().ok())
    {
        rate.reset = Some(n);
    }
    let retry = response
        .header("retry-after")
        .and_then(|s| s.parse::<i64>().ok())
        .map(|n| now.saturating_add(n.clamp(1, 604800)));
    let rate_error = response.status == 429
        || (response.status == 403
            && (rate.remaining == Some(0)
                || retry.is_some()
                || response
                    .json()
                    .ok()
                    .and_then(|v| {
                        v["message"]
                            .as_str()
                            .map(|s| s.to_ascii_lowercase().contains("rate limit"))
                    })
                    .unwrap_or(false)));
    if rate_error {
        rate.cooldown = retry
            .unwrap_or(now.saturating_add(60))
            .max(if rate.remaining == Some(0) {
                rate.reset.unwrap_or(now)
            } else {
                now
            });
        return (
            meta(
                FetchState::RateLimited,
                "GitHub has limited requests. Saved data is retained; wait until the indicated retry time.",
                &old,
                rate,
                url,
                now,
            ),
            old,
        );
    }
    if response.status == 304 {
        if let Some(cache) = old.as_mut() {
            cache.checked = now;
        }
        let (state, message) = if old.is_some() {
            (
                FetchState::NotModified,
                "GitHub confirmed the saved response is unchanged.",
            )
        } else {
            (
                FetchState::Failed,
                "GitHub returned unchanged status without a saved response.",
            )
        };
        return (meta(state, message, &old, rate, url, now), old);
    }
    if response.status != 200 {
        let (state, message) = if response.status == 404 {
            (
                FetchState::Unavailable,
                "This repository or resource is unavailable publicly. Saved data is retained.",
            )
        } else {
            (
                FetchState::Failed,
                "GitHub could not serve this request. Saved data is retained; try again later.",
            )
        };
        return (meta(state, message, &old, rate, url, now), old);
    }
    match parse(&response) {
        Ok(data) => {
            let etag = response
                .header("etag")
                .filter(|s| s.len() <= 512 && !s.contains(['\r', '\n']))
                .map(str::to_owned);
            let cache = Some(Cache {
                data,
                observed: now,
                checked: now,
                etag,
            });
            (
                meta(
                    FetchState::Fresh,
                    "Fetched from the public GitHub API.",
                    &cache,
                    rate,
                    url,
                    now,
                ),
                cache,
            )
        }
        Err(message) => (
            meta(FetchState::Failed, &message, &old, rate, url, now),
            old,
        ),
    }
}

#[cfg(test)]
mod tests;
