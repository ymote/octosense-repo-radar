# Repo Radar

A GitHub project watchlist for OctoSense, inspired by a stock watchlist and
OSSInsight's repository dashboards. Developed under `ymote`, with publication
pending, as a reproducible example of an OctoSense app with its own Rust component and network
access. App ID: `org.ymote.reporadar`. Initial acceptance platform: macOS.

## Screens and actions

- Watchlist: search public GitHub projects by name or `owner/repo`; inspect search
  results and pin a project. Saved projects retain their order across restarts.
  Select a project, unpin it, or move it up/down. Start with an honest empty state
  and suggestions the user can search, not preinstalled personal preferences.
- Project workspace: repository description, stars, forks, open issues plus PRs
  (GitHub's combined counter), and last refresh time. Refresh the selected project.
- Activity: native Makepad D3 charts of bounded recent pull-request observations,
  with exact sample size and collection dates. Distinguish open, merged and closed
  without merging. Never label a sample as lifetime totals or daily growth.
- Releases: recent releases and their notes; open the selected release on GitHub.
  Empty release history is a normal state. Use semver only for tags it can parse.
- Glance: publish a compact summary of the selected repository with a route back
  into its workspace. Collapse/reopen preserves selected repository.
- Optional chat: ask about the selected project through the host app agent, using
  reviewed tools that read the same saved watchlist and call the same Rust functions.
  No provider configuration means an explicit unavailable state; search and pin
  remain fully usable.

## Data and architecture

- `api.github.com`: public repository search, metadata, one bounded recent PR page
  and one bounded releases page. No account, token, private repositories or GitHub
  writes. Never fetch arbitrary URLs supplied in a response or agent instruction.
- `github.com`: foreground external links to the actual repository/release.
- Rust component: `serde`/`serde_json` decode and validate API data, `time` parses
  timestamps, `semver` classifies parseable release tags, and the OctoSense SDK's
  WASI HTTP adapter performs bounded requests. Store durable watchlist and caches
  in the app's own private storage. The Rust component owns this state; Splash and
  agent tools are clients, not separate writers of a second copy.
- Search is explicitly submitted, not requested on every keystroke. Cache results
  and honor rate limits. Use one external request per component invocation because
  the current host imposes a per-call deadline. Avoid full-history crawling.
- Do not use OSSInsight's degraded event history as accurate current growth. Any
  future optional historical panel must surface its source's data-quality status.
- No background polling claim: this version refreshes on user actions; the host
  does not currently schedule arbitrary store-app cron triggers.

## States and acceptance

Empty watchlist/search/no releases; loading; invalid input; failed request with
retry; rate limit with reset information; saved data with last-observed date;
missing component or chart host; optional agent unavailable. Do not display D3
demo data when real data is absent. Hide the chart behind an honest state panel.

Drive search, pin, selection, unpin, reorder, refresh, tabs, release links, and
restart persistence using native Makepad instrumentation in an owned test profile.
Check real pixels and keyboard/scroll geometry. Record source and exact host hashes,
preserve failed runs, and separate source-host testing from released-host install
and update acceptance. Do not claim Android until run on a compatible phone release.

## Disclosures

- `net`: public GitHub requests.
- `storage`: watchlist, preferences and bounded cached public data.
- `wasm`: the app's own compiled Rust component.
- `glance`: user-requested project summary in Glancer.
- App agent: optional host-provided conversation and tools, disclosed in listing,
  guidance and privacy text; user/host consent and inter-app sharing remain intact.

Publisher: `ymote`. Source/support: the app's GitHub repository and issue tracker.
No credentials, personal profiles, private email, or publishing private keys enter
the bundle or repository. Use GitHub-attested publishing with an App Hub submission
issue; a release alone does not publish a catalog entry.
