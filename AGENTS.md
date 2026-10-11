# Developing this OctoSense app

`CLAUDE.md` and `GEMINI.md` only import this file, so every coding agent reads
the same rules.

This repository is one OctoSense script app. `bundle/` is the app and the only
thing submitted to the App Hub; everything else stays outside it.

Follow the docs of OctoSense App Flow, the harness this app came from,
and do not invent requirements or APIs:

- Build, run and test: [QUICKSTART](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/QUICKSTART.md)
- The language and every API an app may use: [SCRIPT-API](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/SCRIPT-API.md)
- Capabilities: [CAPABILITIES](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/CAPABILITIES.md)
- Which host API families serve store apps, where and since when: [HOST-API-FAMILIES](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/HOST-API-FAMILIES.md)
- Final bundle, screenshots and human checkpoints: [PUBLISHING](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/PUBLISHING.md)
- Publisher proof and the submission issue: App Hub's [SUBMITTING](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/SUBMITTING.md)

**Repo Radar override:** this app requires a full OctoSense shell with Wasm
components, `host-api-v1`, `charts.d3@1` and the WASI HTTPS provider fix:
[desktop RC5](https://github.com/OctoSense-org/OctoSense/releases/tag/desktop-v0.1.0-rc.5)
is the first release with all of them (RC4 panicked on a component's first
HTTPS request). The standalone card-host loop below cannot run this exact
bundle. Use [tools/NATIVE-ACCEPTANCE.md](tools/NATIVE-ACCEPTANCE.md) and an
isolated full-shell profile. Never remove manifest compatibility requirements
just to pass a preview.

Before a full-shell run, with `OCTO` set as described below, check the component on its own:

1. `cargo test --locked --manifest-path components/radar/Cargo.toml` (18 tests,
   no network).
2. `$OCTO wasm build --app .` rebuilds `bundle/fns/radar.wasm` from
   `components/radar`; with the pinned SDK and Cargo.lock it prints
   `unchanged bundle/fns/radar.wasm` for the published bytes.
3. Call a function under the shell's runtime from an OctoSense checkout, as
   [README § Build and test](README.md#build-and-test) shows
   (`cargo run -p octosense-wasm-host --example wasm_call -- /path/to/octosense-repo-radar/bundle/fns/radar.wasm radar_search '{"query":"…","page":1}' --storage DIR`);
   this makes a real GitHub request and writes the state file in `DIR`.
4. Run `$OCTO check bundle` with a `hub` built from App Hub `main`; expect
   `— PASSED` with the unsigned warning, the component's reach and crate
   list, and the destructive-tool warning for `reporadar.unpin`.

Repo Radar is published (App Hub catalog sequence 16, version 0.1.2), so the
submission rule below for an unpublished app no longer applies: each new
version gets its own App Hub issue.

The following generated loop applies to simpler card-host-compatible apps.
Set `OCTO=<path to OctoSense-App-Flow>/tools/octo`, then run this loop
from this directory. The CLI lives in the harness repository, not here; on
Windows, run it as `python $OCTO`.

1. Edit `bundle/main.splash`.
2. Run `$OCTO run bundle --port 8141 --hidden --detach`. It returns after
   `ready: first frame drawn`. If it exits 1, the port is taken: run the
   `curl … /quit` it prints, then try again.
3. Drive the app (`/click`, `/t`, `/snap`), then run `$OCTO shot 8141 out.png`
   and open the PNG.
4. Run `curl -s 127.0.0.1:8141/quit`.
5. Run `$OCTO check bundle`. Expect `— PASSED` with only the unsigned
   warning. Until `bundle/screenshots/01-main.png` exists
   (`$OCTO shot 8141 bundle/screenshots/01-main.png` in step 3), the gate
   refuses the listing; fix every other `[refused]` line before you go on.

Follow these rules:

- Ask only for capabilities a screen uses; declare every `https://` host in
  `network.hosts`; never use `http://`.
- Before building on a host API, check that its family serves store apps on your platforms ([HOST-API-FAMILIES](https://github.com/OctoSense-org/OctoSense-App-Flow/blob/main/docs/HOST-API-FAMILIES.md)): `llm`, `news` and `calendar` answer only system apps, `clipboard` fails, and `mail` cannot send on RC1.
- Never collect or store a password, PIN, one-time code, API key or token;
  accounts go through a host service.
- Keep the id's last segment, after its final `.`, off App Hub's reserved
  names (`notes`, `weather`, `terminal` and others); the gate refuses them.
- Put only files the gate accepts in `bundle/`: no `.DS_Store`, no file
  without an extension, 8 MiB at most. Plain `.txt`/`.md` license and
  attribution URLs are allowed; keep required notices. Agent guidance and
  structured resources retain their host/resource checks.
- Capture screenshots from the unsigned bundle with `$OCTO shot`, and look at
  each one; never use a placeholder.
- Restamp editable unsigned source after every edit (`$OCTO check` does it).
  Never edit or restamp a sealed attested/signed release. Test and capture
  screenshots before the publishing workflow seals the bundle.
- Keys, `.local-state/`, `build/` and review packets never enter `bundle/` or Git.
- Keep `.gitattributes` (`bundle/** -text`) committed: the digest covers every
  byte, and a line-ending conversion breaks it.
- Review `.github/workflows/publish-app.yml` before releasing.
  `$OCTO publish-github .` installs it for an existing app; `new` copies it too.
  App Hub accepts only GitHub-attested releases, so no publisher key or
  repository signing secret is involved. Never create a publisher key, sign a
  manifest or pass `--publisher-key` to publish. Push a new
  `v<manifest.version>` tag of tested source to prepare, attest, verify and
  pack it. The tag source and sealed release pack are different artifacts.
- Opening the App Hub issue is the request to publish; provide repository,
  version/commit, screenshots and permissions, then add the verified release
  pack when ready. Until App Hub first publishes the app, post each new
  release on the same issue and update its title and Version field; after
  publication, open a new issue for each new version (the rule for Repo Radar since
  0.1.2). A release alone does not submit or approve the app.
- Stop at human steps: publisher details, workflow review, platform claims,
  tag and submission, unless already authorized in this session. Respect
  actual approval; never fabricate it.
- `publisher-github-v1` needs contract 1.8.0 and a compatible host; older
  hosts and `card-host` refuse the sealed release. OctoSense desktop
  0.1.0-rc.1 and later install GitHub-attested apps (verified on macOS), and
  this app's manifest requirements make desktop RC5 its first compatible
  release; no phone platform is claimed for Repo Radar. Follow the linked
  publishing guide for current host-release evidence.

Add this app's own requirements, data sources and tests below.
