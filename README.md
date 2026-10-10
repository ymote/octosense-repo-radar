# Repo Radar

A public GitHub project watchlist for OctoSense. Search projects, pin and reorder
up to 100 favorites, inspect recent pull requests and releases, and return to the same saved
list after restarting. Pinning is local; it does not star the project on GitHub.

This is a work in progress. The bundle is not yet published in App Hub, and
released-host and public-catalog acceptance are still pending. Development
acceptance passed 94 native steps; Glance routing and unsent Chat input were also
checked. These results do not establish live model answers or public installation. Do not treat a source build as a release.

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
and D3 host addition are being validated together before a compatible release;
RC4 is not a working networking baseline. The text fallback is defensive UI,
not a claim of older-host support. Android is not claimed.

PR activity covers one recent page, not all project history. Star changes mean
"since last observed," not daily growth. GitHub's open-issues counter also
includes PRs. Each response includes its observation time, coverage and error or
rate-limit status. OSSInsight's degraded historical feed is not used as accurate
current growth. [BRIEF.md](BRIEF.md) states the acceptance scope.

## Build and test

Use [OctoSense App Flow](https://github.com/OctoSense-org/OctoSense-App-Flow)
at commit `dd25bdb6a3f9e3757b586447e4b5ca57d412deb5` or a compatible newer revision.
The SDK dependency is pinned in the Rust manifest and Cargo.lock. With Rust 1.97.1
and the `wasm32-wasip2` target installed:

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

The bundle holds no private key, account configuration or provider credential.
Publication uses the GitHub-attested workflow, followed by an App Hub submission
issue and catalog review. Pushing a tag alone does not make the app discoverable.

See [PRIVACY.md](PRIVACY.md) for network use, local storage and optional chat.

Development evidence and its limits are recorded in [review/acceptance-2026-10-10.json](review/acceptance-2026-10-10.json).
