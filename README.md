# Repo Radar

A public GitHub project watchlist for OctoSense. Search projects, pin and reorder
up to 100 favorites, inspect recent pull requests and releases, and return to the same saved
list after restarting. Pinning is local; it does not star the project on GitHub.

[Version 0.1.2](https://github.com/ymote/octosense-repo-radar/releases/tag/v0.1.2)
is listed in App Hub. On Apple Silicon Macs, install
[OctoSense Desktop RC5](https://github.com/OctoSense-org/OctoSense/releases/tag/desktop-v0.1.0-rc.5),
open **App Hub → Search**, search **Repo Radar**, and choose **Get → Install**.
No Rust toolchain, GitHub login or model provider is needed to use the watchlist.

The [public-store acceptance](review/acceptance-public-store-0.1.2.json) passed
93 native checks using a fresh, fully downloaded public RC5 package and the
host's built-in store, with no catalog or trust overrides. The installed app
matched all nine sealed release files. This covered live GitHub requests through
the Rust Wasm component, native charts, search, pins, ordering, releases, refresh,
Glance publication and restart persistence. The
[publication receipt](review/publication-0.1.2.json) records the GitHub-attested
catalog transaction and full public file readback; see also
[submission #202](https://github.com/OctoSense-org/OctoSense-App-Hub/issues/202).

Version **0.1.2** carries the 0.1.1 UX revision following OctoSense App Flow:
compact watchlist rows, a complete chart on the opening screen, one consistent
Activity / Releases / Chat row, formatted release notes, and separate unsent
chat drafts for each project. The historical 0.1.0 functional checks are not
visual approval of this revision. The [UX review](review/ux-0.1.1.json) records
native checks, repaired visual defects and remaining limits. The
[sealed 0.1.2 acceptance](review/acceptance-release-0.1.2.json) records the
verified publisher proof, compiled component and complete native flow.
Version 0.1.2 adds the listing's explicit disclosure that live model replies and agent tool
execution remain unverified; its runtime code is identical to 0.1.1.

## Use the watchlist

Once installed in a compatible OctoSense release:

1. Search a project name or `owner/repo`, then select a result.
2. Choose **+ Pin project** to add it to your local watchlist. Choose **Done**
   to leave search results and return to the saved list.
3. Select a pinned project to see its activity and releases. Use **↑ / ↓** to
   reorder it or **Unpin** to remove it. The list and selection survive restarts.
4. Choose **Refresh** for a new observation, or **Glance** to publish a project
   summary whose **Open project** button returns to that project's workspace.

**Chat** uses OctoSense's optional app agent when consent and a model provider
are configured. Search, charts and the watchlist do not require it. This version
refreshes on user actions; it does not monitor projects in the background.
Live model replies and agent tool execution have not yet been validated.
Chat drafts stay with their project while switching tabs and projects
during the current app session; unsent drafts are not persisted across restarts.
If assistant consent is off, the draft stays editable and sending is disabled.
Release notes use native Markdown; use the release title or full-notes link to
open GitHub. Inline links inside the preview are rendered as text because of a
Markdown-link defect in the tested host.

## Architecture

`bundle/main.splash` draws the native UI. `components/radar` compiles Rust and
crates into a Wasm component. The host runs its exports asynchronously; the Rust
component uses the SDK's WASI HTTP adapter to query public GitHub endpoints and
owns the app's durable state. No native library installation, JavaScript backend,
GitHub login or token is required. [API.md](API.md) describes the typed boundary.

The Rust dependencies have distinct jobs: `serde` and `serde_json` normalize
GitHub responses, `time` handles timestamps and cooldowns, and `semver` identifies
release tags it understands while preserving other tags. The release workflow
rebuilds the component from its tagged source and records its crate inventory.

Native charts use Makepad D3. The current manifest requires `host-api-v1` and
`charts.d3@1` alongside Wasm component support, so RC4 cannot install it. Desktop RC4
loads the Rust component, but live acceptance found that its WASI HTTPS path
panics when choosing a TLS provider. It also does not register D3. The HTTPS fix
and D3 host addition ship in RC5, which passed the public-store acceptance.
A compatible host runs this app's published Wasm component without recompiling
OctoSense. RC4 is not a working networking baseline. The text fallback is
defensive UI, not a claim of older-host support. Android is not claimed.

PR activity covers one recent page, not all project history. Star changes mean
"since last observed," not daily growth. GitHub's open-issues counter also
includes PRs. Each response includes its observation time, coverage and error or
rate-limit status. OSSInsight's degraded historical feed is not used as accurate
current growth. [BRIEF.md](BRIEF.md) states the acceptance scope.

## Build and test

Use [OctoSense App Flow](https://github.com/OctoSense-org/OctoSense-App-Flow)
at commit `dd25bdb6a3f9e3757b586447e4b5ca57d412deb5` or a compatible newer revision;
App Flow `main` at `27df17f3` (11 October 2026) rebuilt the published 0.1.2
component byte for byte. The SDK dependency is pinned in the Rust manifest and
Cargo.lock. With Rust 1.97.1 and the `wasm32-wasip2` target installed:

```sh
cargo test --locked --manifest-path components/radar/Cargo.toml
RUSTFLAGS="--remap-path-prefix=$HOME=/builder --remap-path-prefix=$PWD=/app" \
  python /path/to/OctoSense-App-Flow/tools/octo wasm build --app .
python /path/to/OctoSense-App-Flow/tools/octo check bundle
```

Before building a distributable component, set Rust's `--remap-path-prefix` flags
for the checkout and home directory so panic metadata does not embed private
local paths. The publishing workflow does this and rebuilds with `--locked`.

`card-host` cannot run this bundle: it lacks the required host APIs and Wasm
service. Use a compatible full OctoSense shell and the
[native acceptance guide](tools/NATIVE-ACCEPTANCE.md). Native acceptance tools
and dated receipts distinguish an isolated development catalog from actual
GitHub-attested publication and released-host installation.

The component's functions can be called from the command line, without the
shell, through OctoSense's `wasm_call` example (OctoSense `main` since
[#455](https://github.com/OctoSense-org/OctoSense/pull/455)). From an
OctoSense checkout with its pinned sources set up:

```sh
cargo run -q -p octosense-wasm-host --example wasm_call -- \
  /path/to/octosense-repo-radar/bundle/fns/radar.wasm \
  radar_search '{"query":"octosense","page":1}' --storage /tmp/radar-state
```

That runs the component under the same runtime the shell uses, with a real
HTTPS request to `api.github.com` and the watchlist file written to the
storage folder. On 11 October 2026 the published 0.1.2 component returned a
`fresh` first page this way. Host services are refused there (`needs a
shell`), so Glance and chat tools still need the full shell.

The build and the gate were last rechecked on 11 October 2026: `cargo test
--locked` in `components/radar` passed 18 tests, `octo wasm build` with App
Flow `main` reproduced `bundle/fns/radar.wasm` unchanged, and `octo check
bundle` with App Hub `main` (`fb70ddd`) printed `— PASSED` with the
expected warnings (unsigned source, the component's reach and its 18
crates, the destructive `reporadar.unpin` tool).

The bundle holds no private key, account configuration or provider credential.
Publication uses the GitHub-attested workflow, followed by an App Hub submission
issue and catalog review. Pushing a tag alone does not make the app discoverable.

See [PRIVACY.md](PRIVACY.md) for network use, local storage and optional chat.

## Acceptance limits

The public-store run established the functional path above, not a blanket UX or
performance pass. It recorded nine UI stalls, the longest 3,730 ms. One screenshot
omitted a PR title even though the cache and widget tree contained it. A separate
[42-step focused run](review/title-observation-0.1.2.json) with the same released
host and app displayed that title in its initial, delayed and scroll-return
frames. The original omission is preserved and its cause remains unresolved.
The focused run also recorded seven stalls, the longest 9,020 ms; responsiveness
still needs investigation.

Live model replies, a second-version app update, Android, physical input,
external-browser navigation and activating Glance's **Open project** button were
not verified by this public-store run. Optional Chat remains a preview.

Release provenance: [full anonymous Mac download](review/host-rc5-public-download.json),
[final Mac bytes](review/host-rc5-final-macos-binding.json), and
[host source checks](review/host-rc5-source-ci.json). Earlier
[packaged RC5 acceptance](review/acceptance-rc5-candidate-0.1.2.json) used a signed
catalog rehearsal; it is retained separately from the public-store result.

Evidence and its limits: [current sealed-release checks](review/acceptance-release-0.1.2.json),
[source development checks](review/acceptance-2026-10-10.json), and
[historical 0.1.0 checks](review/acceptance-release-0.1.0.json).
