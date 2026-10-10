use super::*;
use crate::storage::STATE_FILE;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::VecDeque,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQ: AtomicU64 = AtomicU64::new(0);
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "repo-radar-test-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
struct Fake {
    responses: RefCell<VecDeque<Result<Wire, String>>>,
    calls: RefCell<Vec<(String, Option<String>)>>,
}
impl Fake {
    fn with(responses: Vec<Wire>) -> Self {
        Self {
            responses: RefCell::new(responses.into_iter().map(Ok).collect()),
            ..Default::default()
        }
    }
}
impl Transport for Fake {
    fn get(&self, url: &str, etag: Option<&str>) -> Result<Wire, String> {
        self.calls
            .borrow_mut()
            .push((url.into(), etag.map(str::to_owned)));
        self.responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected extra HTTP call")
    }
}
fn wire(status: u16, v: Value) -> Wire {
    Wire {
        status,
        headers: vec![],
        body: serde_json::to_vec(&v).unwrap(),
    }
}
fn repo(id: u64, name: &str, stars: u64) -> Value {
    json!({"id":id,"full_name":name,"private":false,"description":"A project","stargazers_count":stars,"forks_count":4,"open_issues_count":7,"subscribers_count":2,"language":null,"pushed_at":"2026-10-10T01:00:00Z","created_at":"2020-01-01T00:00:00Z","default_branch":"main","license":null,"topics":["rust"],"html_url":"https://evil.example/phishing"})
}
fn search(items: Vec<Value>) -> Wire {
    wire(
        200,
        json!({"items":items,"total_count":20,"incomplete_results":false}),
    )
}
const NOW: i64 = 1791597600;

#[test]
fn public_results_are_normalized_and_link_targets_are_not_trusted() {
    let r = provider::repo(&repo(1, "makepad/makepad", 10)).unwrap();
    assert_eq!(r.html_url, "https://github.com/makepad/makepad");
    assert_eq!(r.language, "");
    assert_eq!(r.open_issues_and_prs, 7);
    let mut v = repo(2, "owner/name", 1);
    v["private"] = json!(true);
    assert!(provider::repo(&v).is_err());
    v["private"] = Value::Null;
    assert!(provider::repo(&v).is_err());
    for name in [
        "https://evil.example/a",
        "owner/../repo",
        "owner/..",
        "owner/repo?access=x",
        "owner/repo#tag",
        "owner/repo%2Fx",
    ] {
        assert!(provider::full_name(name).is_err(), "{name}");
    }
    assert_eq!(
        provider::encode("rust language:Rust &中文"),
        "rust%20language%3ARust%20%26%E4%B8%AD%E6%96%87"
    );
}
#[test]
fn parsers_distinguish_merged_draft_closed_and_bound_unicode_release_notes() {
    let base = json!({"number":1,"title":"fix","state":"closed","draft":false,"created_at":"2026-10-01T01:00:00Z","updated_at":"2026-10-02T01:00:00Z","closed_at":"2026-10-02T01:00:00Z","merged_at":"2026-10-02T01:00:00Z","user":{"login":"contributor"},"labels":[]});
    let mut closed = base.clone();
    closed["merged_at"] = Value::Null;
    let mut draft = base.clone();
    draft["state"] = json!("open");
    draft["draft"] = json!(true);
    draft["merged_at"] = Value::Null;
    let rows = provider::pulls(&json!([base, closed, draft]), "o/r").unwrap();
    assert_eq!(
        rows.iter().map(|p| p.state.clone()).collect::<Vec<_>>(),
        [PullState::Merged, PullState::Closed, PullState::Draft]
    );
    let release = json!({"id":7,"tag_name":"v1.2.0-rc.10","name":null,"published_at":"2026-10-10T01:00:00Z","prerelease":true,"body":"文".repeat(2050),"html_url":"file:///bad"});
    let mut unparsed = release.clone();
    unparsed["tag_name"] = json!("nightly/branch");
    let values = provider::releases(&json!([release, unparsed]), "o/r").unwrap();
    assert_eq!(values[0].parsed_semver.as_deref(), Some("1.2.0-rc.10"));
    assert_eq!(values[0].body_preview.chars().count(), 2048);
    assert!(values[0].body_truncated);
    assert_eq!(values[1].parsed_semver, None);
    assert_eq!(
        values[1].html_url,
        "https://github.com/o/r/releases/tag/nightly%2Fbranch"
    );
}
#[test]
fn watchlist_persistence_selection_reorder_and_unpin_use_stable_ids() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![search(vec![
            repo(10, "a/one", 1),
            repo(20, "a/two", 2),
            repo(30, "a/three", 3),
        ])]),
        NOW,
    )
    .unwrap();
    assert!(app.watchlist().pins.is_empty());
    assert!(app.pin("a/one").is_err());
    app.search("a", 1).unwrap();
    app.pin("a/one").unwrap();
    app.pin("a/two").unwrap();
    app.pin("a/three").unwrap();
    assert_eq!(app.pin("A/TWO").unwrap().pins.len(), 3);
    app.move_pin(30, -1).unwrap();
    app.select(30).unwrap();
    assert!(app.select(99).is_err());
    assert!(app.move_pin(30, 2).is_err());
    drop(app);
    let mut resumed = App::new(&dir.0, Fake::default(), NOW + 1).unwrap();
    assert_eq!(
        resumed
            .watchlist()
            .pins
            .iter()
            .map(|p| p.repo.id)
            .collect::<Vec<_>>(),
        [10, 30, 20]
    );
    assert_eq!(resumed.watchlist().selected_repo_id, Some(30));
    let w = resumed.unpin(30).unwrap();
    assert_eq!(w.selected_repo_id, Some(20));
    assert_eq!(w.pins.len(), 2);
    assert!(resumed.transport.calls.borrow().is_empty());
    let before = std::fs::read(dir.0.join(STATE_FILE)).unwrap();
    assert!(resumed.pin("new/unverified").is_err());
    assert_eq!(std::fs::read(dir.0.join(STATE_FILE)).unwrap(), before);
}
#[test]
fn pin_limit_is_enforced_and_corruption_is_never_overwritten() {
    let dir = Dir::new();
    let mut app = App::new(&dir.0, Fake::default(), NOW).unwrap();
    for id in 1..=101 {
        app.store.remember(
            provider::repo(&repo(id, &format!("o/r{id}"), id)).unwrap(),
            NOW,
        );
    }
    for id in 1..=100 {
        app.pin(&format!("o/r{id}")).unwrap();
    }
    let saved = std::fs::read(dir.0.join(STATE_FILE)).unwrap();
    assert!(app.pin("o/r101").is_err());
    assert_eq!(std::fs::read(dir.0.join(STATE_FILE)).unwrap(), saved);
    std::fs::write(dir.0.join(STATE_FILE), b"{broken").unwrap();
    assert!(Store::load(&dir.0).is_err());
    assert_eq!(std::fs::read(dir.0.join(STATE_FILE)).unwrap(), b"{broken");
}
#[test]
fn ttl_force_conditional_refresh_and_observation_deltas_are_truthful() {
    let dir = Dir::new();
    let mut first = wire(200, repo(1, "o/repo", 100));
    first.headers.push(("ETag".into(), "\"one\"".into()));
    let mut second = repo(1, "o/repo", 98);
    second["forks_count"] = json!(6);
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![first, wire(304, Value::Null), wire(200, second)]),
        NOW,
    )
    .unwrap();
    let fresh = app.overview("o/repo", false).unwrap();
    assert_eq!(fresh.meta.state, FetchState::Fresh);
    assert_eq!(fresh.value.as_ref().unwrap().star_delta, None);
    app.now += 2;
    assert_eq!(
        app.overview("o/repo", false).unwrap().meta.state,
        FetchState::Cached
    );
    assert_eq!(app.transport.calls.borrow().len(), 1);
    let unchanged = app.overview("o/repo", true).unwrap();
    assert_eq!(unchanged.meta.state, FetchState::NotModified);
    assert_eq!(unchanged.meta.fetched_at, fresh.meta.fetched_at);
    assert_eq!(unchanged.value.unwrap().snapshots.len(), 1);
    assert_eq!(
        app.transport.calls.borrow()[1].1.as_deref(),
        Some("\"one\"")
    );
    app.now += 60;
    let changed = app.overview("o/repo", true).unwrap().value.unwrap();
    assert_eq!(changed.star_delta, Some(-2));
    assert_eq!(changed.fork_delta, Some(2));
    assert_eq!(changed.previous_observed_at, Some(provider::timestamp(NOW)));
    assert_eq!(changed.snapshots.len(), 2);
}
#[test]
fn rate_limit_survives_restart_and_force_does_not_bypass_it() {
    let dir = Dir::new();
    let mut limited = wire(429, json!({"message":"rate limit"}));
    limited.headers = vec![
        ("Retry-After".into(), "120".into()),
        ("X-RateLimit-Remaining".into(), "0".into()),
        ("X-RateLimit-Reset".into(), (NOW + 180).to_string()),
    ];
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![wire(200, repo(1, "o/r", 3)), limited]),
        NOW,
    )
    .unwrap();
    app.overview("o/r", false).unwrap();
    app.now += 1;
    let result = app.overview("o/r", true).unwrap();
    assert_eq!(result.meta.state, FetchState::RateLimited);
    assert!(result.meta.cached);
    assert_eq!(result.meta.retry_at, Some(provider::timestamp(NOW + 180)));
    drop(app);
    let mut resumed = App::new(&dir.0, Fake::default(), NOW + 30).unwrap();
    let result = resumed.overview("o/r", true).unwrap();
    assert_eq!(result.meta.state, FetchState::RateLimited);
    assert_eq!(result.value.unwrap().repo.stars, 3);
    assert!(resumed.transport.calls.borrow().is_empty());
}
#[test]
fn invalid_inputs_never_request_and_search_budget_is_independent() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![search(vec![repo(1, "o/r", 3)])]),
        NOW,
    )
    .unwrap();
    app.store.state.core_rate.cooldown = NOW + 500;
    for (q, p) in [("", 1), ("x\ny", 1), ("rust", 0), ("rust", 101)] {
        assert!(app.search(q, p).is_err());
    }
    assert!(app.overview("https://evil.example/a", true).is_err());
    assert!(app.pulls("unseen/repo", true).is_err());
    assert!(app.transport.calls.borrow().is_empty());
    assert_eq!(
        app.search("rust language:Rust", 1).unwrap().meta.state,
        FetchState::Fresh
    );
    let url = &app.transport.calls.borrow()[0].0;
    assert_eq!(
        url,
        "https://api.github.com/search/repositories?q=rust%20language%3ARust&per_page=10&page=1"
    );
}
#[test]
fn malformed_or_failed_refresh_keeps_saved_data_and_first_failure_is_empty() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![
            wire(200, repo(1, "o/r", 8)),
            Wire {
                status: 200,
                headers: vec![],
                body: b"bad json".to_vec(),
            },
            wire(404, Value::Null),
        ]),
        NOW,
    )
    .unwrap();
    app.overview("o/r", false).unwrap();
    app.now += 1;
    let bad = app.overview("o/r", true).unwrap();
    assert_eq!(bad.meta.state, FetchState::Failed);
    assert!(bad.meta.cached);
    assert_eq!(bad.value.unwrap().repo.stars, 8);
    let missing = app.overview("other/missing", false).unwrap();
    assert_eq!(missing.meta.state, FetchState::Unavailable);
    assert!(missing.value.is_none());
    assert!(!missing.meta.cached);
    let oversized = Wire {
        status: 200,
        headers: vec![],
        body: vec![b' '; provider::BODY_LIMIT + 1],
    };
    assert!(oversized.json().unwrap_err().contains("2 MiB"));
}
#[test]
fn cache_pruning_keeps_pins_and_observation_history_is_bounded() {
    let dir = Dir::new();
    let mut app = App::new(&dir.0, Fake::default(), NOW).unwrap();
    app.store
        .remember(provider::repo(&repo(1, "o/r1", 1)).unwrap(), NOW);
    app.pin("o/r1").unwrap();
    for id in 2..=150 {
        app.store.remember(
            provider::repo(&repo(id, &format!("o/r{id}"), id)).unwrap(),
            NOW + id as i64,
        );
    }
    app.store.save().unwrap();
    assert_eq!(app.store.state.repos.len(), 128);
    assert!(app.store.state.repos.contains_key(&1));
    let responses = (0..100).map(|n| wire(200, repo(1, "o/r1", n))).collect();
    let mut app = App::new(&dir.0, Fake::with(responses), NOW + 100).unwrap();
    for _ in 0..100 {
        app.overview("o/r1", true).unwrap();
        app.now += 1;
    }
    assert_eq!(app.store.state.repos[&1].snapshots.len(), 96);
    assert_eq!(
        app.store.state.repos[&1]
            .overview
            .as_ref()
            .unwrap()
            .data
            .snapshots
            .len(),
        96
    );
}
#[test]
fn rename_keeps_pin_identity_and_public_enum_json_is_snake_case() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![
            search(vec![repo(1, "o/old", 2)]),
            wire(200, repo(1, "o/new", 3)),
        ]),
        NOW,
    )
    .unwrap();
    app.search("o", 1).unwrap();
    app.pin("o/old").unwrap();
    app.overview("o/old", true).unwrap();
    assert_eq!(app.watchlist().pins[0].repo.full_name, "o/new");
    assert_eq!(app.watchlist().selected_repo_id, Some(1));
    assert_eq!(app.pin("o/new").unwrap().pins.len(), 1);
    assert_eq!(
        serde_json::to_string(&FetchState::RateLimited).unwrap(),
        "\"rate_limited\""
    );
    assert_eq!(
        serde_json::to_string(&PullState::Merged).unwrap(),
        "\"merged\""
    );
}

#[test]
fn bounded_pr_and_release_pages_have_independent_caches_and_no_followup_requests() {
    let dir = Dir::new();
    let pull = json!({"number":2,"title":"Change","state":"open","draft":false,"created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-09T00:00:00Z","merged_at":null,"closed_at":null,"user":{"login":"dev"},"labels":[]});
    let mut pulls = wire(200, json!(vec![pull; 12]));
    pulls.headers.push((
        "link".into(),
        "<https://evil.example/steal>; rel=\"next\"".into(),
    ));
    let releases = wire(200, json!([]));
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![search(vec![repo(1, "o/r", 3)]), pulls, releases]),
        NOW,
    )
    .unwrap();
    app.search("o/r", 1).unwrap();
    let p = app.pulls("o/r", false).unwrap();
    assert_eq!(p.items.len(), 10);
    assert_eq!(p.page.next_page, Some(2));
    assert_eq!(p.page.total_count, None);
    assert!(p.page.coverage.contains("bounded sample"));
    let r = app.releases("o/r", false).unwrap();
    assert_eq!(r.meta.state, FetchState::Fresh);
    assert!(r.items.is_empty());
    assert_eq!(
        app.pulls("o/r", false).unwrap().meta.state,
        FetchState::Cached
    );
    assert_eq!(
        app.releases("o/r", false).unwrap().meta.state,
        FetchState::Cached
    );
    assert_eq!(app.transport.calls.borrow().len(), 3);
    assert!(
        app.transport
            .calls
            .borrow()
            .iter()
            .all(|(url, _)| url.starts_with("https://api.github.com/"))
    );
}

#[test]
fn failed_atomic_replace_retains_previous_watchlist_and_pending_file_is_not_loaded() {
    let dir = Dir::new();
    let mut app = App::new(&dir.0, Fake::default(), NOW).unwrap();
    app.store
        .remember(provider::repo(&repo(1, "o/r", 1)).unwrap(), NOW);
    app.pin("o/r").unwrap();
    let previous = std::fs::read(dir.0.join(STATE_FILE)).unwrap();
    // A blocked staging path deterministically fails before atomic replacement.
    std::fs::create_dir(dir.0.join("repo-radar-v1.pending.json")).unwrap();
    assert!(app.unpin(1).is_err());
    assert_eq!(std::fs::read(dir.0.join(STATE_FILE)).unwrap(), previous);
    let loaded = Store::load(&dir.0).unwrap();
    assert_eq!(loaded.watchlist().pins.len(), 1);
    assert_eq!(loaded.watchlist().selected_repo_id, Some(1));
}

#[test]
fn search_cache_prunes_oldest_pages_and_can_restore_verified_evicted_summaries() {
    let dir = Dir::new();
    let responses = (1..=9)
        .map(|id| search(vec![repo(id, &format!("o/r{id}"), id)]))
        .collect();
    let mut app = App::new(&dir.0, Fake::with(responses), NOW).unwrap();
    for page in 1..=9 {
        app.search("rust", page).unwrap();
        app.now += 1;
    }
    assert_eq!(app.store.state.searches.len(), 8);
    assert!(!app.store.state.searches.contains_key("rust\n1"));
    app.store.state.repos.remove(&9);
    let result = app.search("rust", 9).unwrap();
    assert_eq!(result.meta.state, FetchState::Cached);
    assert_eq!(app.pin("o/r9").unwrap().selected_repo_id, Some(9));
    assert_eq!(app.transport.calls.borrow().len(), 9);
}

#[test]
fn secondary_rate_limit_and_elapsed_budget_have_no_hidden_retries() {
    let dir = Dir::new();
    let mut secondary = wire(
        403,
        json!({"message":"You have exceeded a secondary rate limit."}),
    );
    secondary
        .headers
        .push(("x-ratelimit-remaining".into(), "51".into()));
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![secondary, wire(200, repo(1, "o/r", 2))]),
        NOW,
    )
    .unwrap();
    let limited = app.overview("o/r", false).unwrap();
    assert_eq!(limited.meta.state, FetchState::RateLimited);
    assert!(!limited.meta.cached);
    assert_eq!(limited.meta.retry_at, Some(provider::timestamp(NOW + 60)));
    app.now = NOW + 59;
    assert_eq!(
        app.overview("o/r", true).unwrap().meta.state,
        FetchState::RateLimited
    );
    assert_eq!(app.transport.calls.borrow().len(), 1);
    app.now = NOW + 60;
    assert_eq!(
        app.overview("o/r", true).unwrap().meta.state,
        FetchState::Fresh
    );
    assert_eq!(app.transport.calls.borrow().len(), 2);
}

#[test]
fn older_cached_search_does_not_reverse_a_newer_canonical_name() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![
            search(vec![repo(1, "o/old", 1)]),
            wire(200, repo(1, "o/new", 2)),
        ]),
        NOW,
    )
    .unwrap();
    app.search("owner", 1).unwrap();
    app.pin("o/old").unwrap();
    app.now += 10;
    app.overview("o/old", true).unwrap();
    app.now += 10;
    app.search("owner", 1).unwrap();
    assert_eq!(app.watchlist().pins[0].repo.full_name, "o/new");
    assert_eq!(app.pin("o/old").unwrap().selected_repo_id, Some(1));
    assert_eq!(
        app.overview("o/new", false)
            .unwrap()
            .value
            .unwrap()
            .repo
            .full_name,
        "o/new"
    );
}

#[test]
fn maximum_legal_payloads_evict_optional_caches_not_a_hundred_pins() {
    let dir = Dir::new();
    let mut app = App::new(&dir.0, Fake::default(), NOW).unwrap();
    let wide = "😀";
    for id in 1..=128 {
        let mut value = repo(id, &format!("o/r{id}"), u64::MAX);
        value["description"] = json!(wide.repeat(512));
        value["language"] = json!(wide.repeat(64));
        value["forks_count"] = json!(u64::MAX);
        value["open_issues_count"] = json!(u64::MAX);
        value["topics"] = json!(vec![wide.repeat(64); 20]);
        value["license"] = json!({"spdx_id":wide.repeat(64)});
        value["default_branch"] = json!(wide.repeat(128));
        let summary = provider::repo(&value).unwrap();
        app.store.remember(summary, NOW + id as i64);
        let mut overview = provider::overview(&value, NOW + id as i64).unwrap();
        overview.snapshots = (0..96)
            .map(|n| Snapshot {
                observed_at: provider::timestamp(NOW - n),
                stars: u64::MAX,
                forks: u64::MAX,
                open_issues_and_prs: u64::MAX,
            })
            .collect();
        let release=provider::releases(&json!(vec![json!({"id":id,"tag_name":"v1.0.0","name":wide.repeat(256),"body":wide.repeat(2048),"published_at":"2026-10-10T00:00:00Z"});5]),&format!("o/r{id}")).unwrap();
        let pull=provider::pulls(&json!(vec![json!({"number":1,"title":wide.repeat(256),"state":"open","created_at":"2026-10-01T00:00:00Z","updated_at":"2026-10-10T00:00:00Z","user":{"login":wide.repeat(64)},"labels":vec![json!({"name":wide.repeat(64)});10]});10]),&format!("o/r{id}")).unwrap();
        let cache = app.store.state.repos.get_mut(&id).unwrap();
        cache.snapshots = overview.snapshots.clone();
        cache.overview = Some(Cache {
            data: overview,
            observed: NOW + id as i64,
            checked: NOW + id as i64,
            etag: None,
        });
        cache.pulls = Some(Cache {
            data: List {
                page: provider::page(10, None, None, false, "bounded sample"),
                items: pull,
            },
            observed: NOW + id as i64,
            checked: NOW + id as i64,
            etag: None,
        });
        cache.releases = Some(Cache {
            data: List {
                page: provider::page(5, None, None, false, "bounded releases"),
                items: release,
            },
            observed: NOW + id as i64,
            checked: NOW + id as i64,
            etag: None,
        });
        if id < 100 {
            app.store.state.pins.push(SavedPin {
                repo_id: id,
                pinned_at: provider::timestamp(NOW),
            });
        }
    }
    app.store.state.selected_repo_id = Some(42);
    let items = app
        .store
        .state
        .repos
        .values()
        .take(10)
        .map(|c| c.summary.clone())
        .collect::<Vec<_>>();
    for page in 1..=8 {
        app.store.state.searches.insert(
            format!("test\n{page}"),
            Cache {
                data: List {
                    items: items.clone(),
                    page: provider::page(10, None, None, false, "search sample"),
                },
                observed: NOW - 1,
                checked: NOW - 1,
                etag: None,
            },
        );
    }
    assert!(serde_json::to_vec(&app.store.state).unwrap().len() > 4 * 1024 * 1024);
    // A legitimate hundredth pin must succeed despite full optional caches.
    let result = app.pin("o/r100").unwrap();
    assert_eq!(result.pins.len(), 100);
    assert_eq!(result.selected_repo_id, Some(100));
    assert_eq!(
        result.pins.iter().map(|p| p.repo.id).collect::<Vec<_>>(),
        (1..=100).collect::<Vec<_>>()
    );
    assert!(std::fs::metadata(dir.0.join(STATE_FILE)).unwrap().len() <= 4 * 1024 * 1024);
    assert!(app.store.state.searches.is_empty());
    let old = &app.store.state.repos[&1];
    assert!(old.pulls.is_none());
    assert!(old.releases.is_none());
    assert!(old.snapshots.is_empty());
    let newest = &app.store.state.repos[&128];
    assert!(newest.pulls.is_some());
    assert!(newest.releases.is_some());
    assert!(!newest.snapshots.is_empty());
    drop(app);
    let mut resumed = App::new(&dir.0, Fake::default(), NOW + 200).unwrap();
    assert_eq!(resumed.watchlist(), result);
    resumed.select(42).unwrap();
    resumed.move_pin(42, -1).unwrap();
    let expected = resumed.watchlist();
    drop(resumed);
    let resumed = App::new(&dir.0, Fake::default(), NOW + 201).unwrap();
    assert_eq!(resumed.watchlist(), expected);
    assert_eq!(resumed.watchlist().selected_repo_id, Some(42));
}

#[test]
fn displayed_verified_search_can_pin_after_its_unpinned_summary_was_evicted() {
    let dir = Dir::new();
    let mut app = App::new(
        &dir.0,
        Fake::with(vec![search(vec![repo(1, "o/r", 3)])]),
        NOW,
    )
    .unwrap();
    app.search("o/r", 1).unwrap();
    app.store.state.repos.remove(&1);
    app.store.save().unwrap();
    drop(app);
    let mut resumed = App::new(&dir.0, Fake::default(), NOW + 1).unwrap();
    let pins = resumed.pin("o/r").unwrap();
    assert_eq!(pins.pins.len(), 1);
    assert_eq!(pins.pins[0].repo.id, 1);
    assert_eq!(pins.selected_repo_id, Some(1));
    assert!(resumed.transport.calls.borrow().is_empty());
    assert!(resumed.pin("arbitrary/unverified").is_err());
}
