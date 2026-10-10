//! Public GitHub snapshots and one canonical private watchlist. See API.md.
mod engine;
mod provider;
mod storage;

#[octosense_component::export]
pub mod functions {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum FetchState {
        Fresh,
        Cached,
        NotModified,
        RateLimited,
        Unavailable,
        Failed,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum PullState {
        Open,
        Draft,
        Merged,
        Closed,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Meta {
        pub state: FetchState,
        pub message: String,
        pub fetched_at: String,
        pub cached: bool,
        pub rate_remaining: Option<u32>,
        pub retry_at: Option<String>,
        pub source_url: String,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Page {
        pub returned: u32,
        pub next_page: Option<u32>,
        pub total_count: Option<u64>,
        pub incomplete: bool,
        pub coverage: String,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Repo {
        pub id: u64,
        pub full_name: String,
        pub description: String,
        pub html_url: String,
        pub language: String,
        pub stars: u64,
        pub forks: u64,
        pub open_issues_and_prs: u64,
        pub archived: bool,
        pub pushed_at: Option<String>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Snapshot {
        pub observed_at: String,
        pub stars: u64,
        pub forks: u64,
        pub open_issues_and_prs: u64,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Overview {
        pub repo: Repo,
        pub subscribers: Option<u64>,
        pub default_branch: String,
        pub license: String,
        pub topics: Vec<String>,
        pub created_at: String,
        pub observed_at: String,
        pub previous_observed_at: Option<String>,
        pub star_delta: Option<i64>,
        pub fork_delta: Option<i64>,
        pub snapshots: Vec<Snapshot>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SearchResponse {
        pub meta: Meta,
        pub page: Page,
        pub items: Vec<Repo>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct OverviewResponse {
        pub meta: Meta,
        pub value: Option<Overview>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Pull {
        pub number: u64,
        pub title: String,
        pub html_url: String,
        pub author: String,
        pub state: PullState,
        pub created_at: String,
        pub updated_at: String,
        pub closed_at: Option<String>,
        pub merged_at: Option<String>,
        pub labels: Vec<String>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct PullResponse {
        pub meta: Meta,
        pub page: Page,
        pub items: Vec<Pull>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Release {
        pub id: u64,
        pub tag: String,
        pub name: String,
        pub html_url: String,
        pub published_at: Option<String>,
        pub prerelease: bool,
        pub body_preview: String,
        pub body_truncated: bool,
        pub parsed_semver: Option<String>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ReleaseResponse {
        pub meta: Meta,
        pub page: Page,
        pub items: Vec<Release>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Pin {
        pub repo: Repo,
        pub pinned_at: String,
        pub last_observed_at: Option<String>,
        pub star_delta: Option<i64>,
        pub fork_delta: Option<i64>,
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Watchlist {
        pub schema: u32,
        pub revision: u64,
        pub selected_repo_id: Option<u64>,
        pub pins: Vec<Pin>,
    }

    pub fn radar_search(query: String, page: u32) -> Result<SearchResponse, String> {
        super::engine::with_app(|app| app.search(&query, page))
    }
    pub fn radar_overview(full_name: String, force: bool) -> Result<OverviewResponse, String> {
        super::engine::with_app(|app| app.overview(&full_name, force))
    }
    pub fn radar_pulls(full_name: String, force: bool) -> Result<PullResponse, String> {
        super::engine::with_app(|app| app.pulls(&full_name, force))
    }
    pub fn radar_releases(full_name: String, force: bool) -> Result<ReleaseResponse, String> {
        super::engine::with_app(|app| app.releases(&full_name, force))
    }
    pub fn radar_watchlist() -> Result<Watchlist, String> {
        super::engine::with_app(|app| Ok(app.watchlist()))
    }
    pub fn radar_pin(full_name: String) -> Result<Watchlist, String> {
        super::engine::with_app(|app| app.pin(&full_name))
    }
    pub fn radar_unpin(repo_id: u64) -> Result<Watchlist, String> {
        super::engine::with_app(|app| app.unpin(repo_id))
    }
    pub fn radar_select(repo_id: u64) -> Result<Watchlist, String> {
        super::engine::with_app(|app| app.select(repo_id))
    }
    pub fn radar_move(repo_id: u64, direction: i32) -> Result<Watchlist, String> {
        super::engine::with_app(|app| app.move_pin(repo_id, direction))
    }
}
