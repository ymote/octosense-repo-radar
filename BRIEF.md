# Repo Radar

A GitHub project watchlist for OctoSense, inspired by a stock watchlist and
OSSInsight's repository dashboards. Developed under `ymote`, with publication
pending, as a reproducible example of an OctoSense app with its own Rust component and network
access. App ID: `org.ymote.reporadar`. Initial acceptance platform: macOS.

## UX revision 0.1.1

The requester rejected the initial UI. Its functional 94-step run did not
establish good default-screen UX: the driver scrolled to reveal a chart that was
partly hidden at launch. Follow App Flow's script-app and native review loop.
Author/driver: Codex; independent reviewer: a separate Codex agent. Use real public
GitHub data in owned hidden macOS instances, with no personal credentials.

- Primary journey: search, inspect, pin, select another project, return to the
  saved watchlist. Give the list compact rows and a persistent selected state.
- At the existing 990×603 client size, reduce the brand header and metric tiles
  so the complete native PR chart, including category labels, is visible on the
  first populated screen without scrolling. Counts and source coverage remain
  honest; put diagnostic prose behind an explicit data-details control.
- Keep one Activity / Releases / Chat navigation row in the same position.
  Keep pinning with the project header and ordering with the watchlist.
- Preserve the active scroll surface during asynchronous refreshes. Keep the
  chat editor mounted across tabs and refreshes, with a clear project context,
  readable unavailable state, and reachable Send action.
- Use concise loading, empty, search-validation, network and rate-limit states.
  Long repository and release titles remain readable; secondary metadata does
  not crowd out primary content. Match control contrast and focus treatment.
- Recheck native search/pin/reorder/unpin, second/last list item, scrolling,
  releases, Glance routing, unsent draft/focus, empty/error and restart states.
  Review actual pixels at the default size and a smaller desktop width.
- Existing 0.1.0 evidence and sealed artifacts remain unchanged. Capture new
  screenshots and source-bound evidence for the revision. Report visual findings
  separately from functional passes; no invented score or human approval.

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
