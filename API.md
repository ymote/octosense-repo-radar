# Repo Radar component API v1

Frozen interface for `components/radar` and `bundle/main.splash`. All calls use
`host.request("wasm.radar_<name>", {arguments}, callback)`. A successful host
callback exposes the record in `r.data`; input/storage errors use `r.error`.
Network failures are successful records with `meta.state` and cached data when
available. Do not discard a previous panel merely because a refresh failed.

| Export | Arguments | Returned record |
| --- | --- | --- |
| `radar_search` | `query: String, page: u32` | `SearchResponse` |
| `radar_overview` | `full_name: String, force: bool` | `OverviewResponse` |
| `radar_pulls` | `full_name: String, force: bool` | `PullResponse` |
| `radar_releases` | `full_name: String, force: bool` | `ReleaseResponse` |
| `radar_watchlist` | none (`{}`) | `Watchlist` |
| `radar_pin` | `full_name: String` | `Watchlist` |
| `radar_unpin` | `repo_id: u64` | `Watchlist` |
| `radar_select` | `repo_id: u64` | `Watchlist` |
| `radar_move` | `repo_id: u64, direction: i32` (`-1` or `1`) | `Watchlist` |

Records and field names are snake_case in Splash. Options are `null` or their
value; arrays are lists. `FetchState` values are strings: `fresh`, `cached`,
`not_modified`, `rate_limited`, `unavailable`, `failed`. `PullState` values are
`open`, `draft`, `merged`, `closed`. Counters use unsigned integers; changes use
signed integers. The `id` of a repository is the stable pin identity.

```text
Meta { state, message:String, fetched_at:String, cached:bool,
       rate_remaining:Option<u32>, retry_at:Option<String>, source_url:String }
Page { returned:u32, next_page:Option<u32>, total_count:Option<u64>,
       incomplete:bool, coverage:String }
Repo { id:u64, full_name:String, description:String, html_url:String,
       language:String, stars:u64, forks:u64, open_issues_and_prs:u64,
       archived:bool, pushed_at:Option<String> }
Snapshot { observed_at:String, stars:u64, forks:u64, open_issues_and_prs:u64 }
Overview { repo:Repo, subscribers:Option<u64>, default_branch:String,
           license:String, topics:Vec<String>, created_at:String,
           observed_at:String, previous_observed_at:Option<String>,
           star_delta:Option<i64>, fork_delta:Option<i64>, snapshots:Vec<Snapshot> }
SearchResponse { meta:Meta, page:Page, items:Vec<Repo> }
OverviewResponse { meta:Meta, value:Option<Overview> }
Pull { number:u64, title:String, html_url:String, author:String,
       state:PullState, created_at:String, updated_at:String,
       closed_at:Option<String>, merged_at:Option<String>, labels:Vec<String> }
PullResponse { meta:Meta, page:Page, items:Vec<Pull> }
Release { id:u64, tag:String, name:String, html_url:String,
          published_at:Option<String>, prerelease:bool, body_preview:String,
          body_truncated:bool, parsed_semver:Option<String> }
ReleaseResponse { meta:Meta, page:Page, items:Vec<Release> }
Pin { repo:Repo, pinned_at:String, last_observed_at:Option<String>,
      star_delta:Option<i64>, fork_delta:Option<i64> }
Watchlist { schema:u32, revision:u64, selected_repo_id:Option<u64>, pins:Vec<Pin> }
```

`Meta.cached` indicates the returned data comes from a saved successful response;
`fetched_at` is its last successful observation, not the failed refresh time.
`retry_at` is a UTC RFC3339 cooldown timestamp, when supplied. Empty lists with
`fresh`/`cached` are valid empty results; an empty list with `failed` is not.

The Rust component alone owns the fixed private state file. Search caches hold
verified GitHub results, so pinning cannot invent a repository. Pinning selects
that repository, deduplicates by ID and preserves saved order. There are at most
100 pins and 96 observations per repository. Unpinning the selected item chooses
the next available pin. Moving past either end is a no-op. A missing selection,
an unknown pin, or pinning an unverified repository returns a clear error.

One external GET is allowed per call. Search returns 10 entries per explicit
page (maximum page 100); PRs return at most 10 recently updated entries; releases
return at most 5 entries from GitHub's first page. These are bounded samples,
not complete history. Core metadata TTL is 15 minutes; PR/release TTL 5 minutes;
search TTL 5 minutes. `force` bypasses freshness but never a provider cooldown.
There is no polling. A 429 or rate-limited 403 pauses the affected provider budget;
previous records remain usable. No tokens, OAuth, private repositories or GitHub
writes are used.

`open_issues_and_prs` must be labelled **Open issues + PRs**. Deltas are **since
last observed** with `previous_observed_at`, never inferred 24h performance.
A first observation or an evicted prior comparison has no delta; label a missing
comparison as "No saved comparison". Release tags remain valid when they do not
parse as SemVer; `prerelease` comes from GitHub. PR charts must show sample size
and collection time. The component never fetches arbitrary URLs from JSON.


The canonical file is `repo-radar-v1.json` in the component's private WASI
preopen. Its schema version is 1. Root fields are `schema`, `revision`,
`selected_repo_id`, `pins`, `repos`, `searches`, `core_rate`, and `search_rate`.
Stored pins are ordered `{repo_id, pinned_at}` records; `repos` is keyed by the
stable numeric ID serialized as a JSON object key. Successful saves write and
sync a staging file, then replace the canonical file. A damaged canonical file
causes an actionable error and is not overwritten. At most 128 repository summaries,
8 search pages, and 4 MiB of state are retained; pinned identities, canonical summaries, saved order and selection are never
removed by cache pruning. Optional endpoint/search responses and observation
history are evicted oldest first to stay within the serialized byte budget,
even for pinned projects. A later request refetches an evicted response; history
charts cover only retained observations. PR/release calls require an already searched or loaded
public project. Each invocation reloads this single state; the OctoSense host
serializes component calls for an app.

Splash owns a generation number for each selection/search and discards callbacks
from superseded work. A stale callback must not replace the active project pane.
Rust returns canonical current watchlist state after each local mutation; there
is no second Splash storage file.

HTTP requests have a 3-second SDK timeout and no automatic retries. The host also
retains its invocation and memory limits. JSON parsing is limited to 2 MiB after
reading the response: the current SDK buffers the body, so this is not a strict
network-transfer limit. Response-provided URLs are never followed. `next_page`
for PRs/releases indicates additional GitHub history exists; v1 does not expose
pagination for those panels.

Public API references: [repository search](https://docs.github.com/en/rest/search/search#search-repositories),
[repository metadata](https://docs.github.com/en/rest/repos/repos#get-a-repository),
[pull requests](https://docs.github.com/en/rest/pulls/pulls#list-pull-requests),
[releases](https://docs.github.com/en/rest/releases/releases#list-releases), and
[rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api).
