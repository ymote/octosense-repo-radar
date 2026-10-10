# Repo Radar Rust component

This app-owned component implements the nine functions in [API.md](../../API.md).
It calls the public GitHub API through `octosense_component::http` and owns the
private watchlist (up to 100 projects) and bounded caches. Pins, order, and
selection survive eviction of older optional response/history data when the
serialized state approaches 4 MiB. No credential or system account is read.

Run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings`
in this directory. Unit tests use disposable directories and a deterministic
HTTP substitute; they do not access a user's app data or make network requests.
The native exported functions intentionally refuse to access the filesystem.
Actual app HTTP, storage and UI require acceptance in a compatible OctoSense host.

Build for `wasm32-wasip2` with Rust 1.88 or newer and that target installed. Before
building a distributable component, remap machine-specific source paths. For
example, this command creates the component without modifying the app bundle:

```sh
RUSTFLAGS="--remap-path-prefix=$HOME=/builder --remap-path-prefix=$PWD=/radar" \
  cargo build --locked --release --target wasm32-wasip2
```

The output is `target/wasm32-wasip2/release/radar.wasm`. App Flow's
`tools/octo wasm build` also records dependencies and copies the component into
`bundle/fns`; use the same path-remapping environment for that build and the
publishing workflow. Scan the final component before publishing. The fixed SDK
revision and `Cargo.lock` keep dependency selection reproducible; path remapping
prevents local Cargo registry paths from appearing in panic strings.

See API.md for cache lifetimes, cooldowns, bounded-sample labels, storage recovery
and the SDK's post-read JSON size limit. The host serializes calls for this app;
only the component writes its canonical state file.
