# Repo Radar review answers

Candidate: `org.ymote.reporadar` 0.1.1. These are the author's answers, not an
App Hub approval or a claim that publication and released-host acceptance passed.
Run `hub scan bundle --packet build/review.json` on the final stamped bundle.

1. **Behavior and listing.** `bundle/main.splash` implements explicit search,
   a private watchlist, selection, pin/unpin, ordering, activity and release tabs,
   optional app-agent chat, and a user-requested Glance summary. The nine Rust
   exports in `components/radar/src/lib.rs` own networking and durable state.
   Pins are local, not GitHub stars. The UI labels PRs as a recent sample and star
   changes as observed comparisons, not daily growth. No background monitoring,
   private repository access or complete activity history is advertised.
   The historical 0.1.0 release passed 94 native steps against live public GitHub.
   The redesigned 0.1.1 has separate scoped evidence in `ux-0.1.1.json`:
   an earlier 93-step flow, final-source rate-limit and interaction checks,
   and independently reviewed native screenshots. These checks do not replace
   acceptance of the new sealed release.
   The first released-host test found a WASI HTTPS initialization bug in RC4;
   the passing development host includes its fix and D3 registration.

2. **Platforms and category.** The listing claims macOS only and productivity.
   Native instrumentation runs on macOS; Android, Windows and Linux are not
   claimed. A compatible full OctoSense shell is required: standalone card-host
   lacks the Wasm service. The manifest requires `host-api-v1` and
   `charts.d3@1` alongside Wasm component support; RC4 cannot install it.
   Text summaries for absent charts are defensive UI, not older-host support.

3. **Capabilities and hosts.** `storage` retains the private watchlist and
   bounded public-data cache. `wasm` executes the shipped Rust component. `net`
   discloses anonymous HTTPS to `api.github.com`; `github.com` links open public
   repository and release pages in the browser. `glance` publishes a summary only after an explicit
   button press and reads the host's queued app route. The three `octos.*`
   methods open the optional app session, retrieve history, and start a turn.
   No credentials, arbitrary URLs, public GitHub writes, analytics endpoint or
   app-operated backend are used. Reading the local Glance route queue does not
   poll GitHub. The manifest's 8 MiB storage allowance accommodates canonical
   state and its atomic staging copy; each is bounded to 4 MiB.

4. **Interface identity.** Repo Radar is branded as its own repository
   watchlist. It does not imitate a system permission prompt, GitHub login,
   payment sheet or OSSInsight. Its public project information identifies
   GitHub as the source. Loading, unavailable and cached states are distinct;
   empty native charts are not instantiated because the underlying chart
   library would substitute demonstration values.

5. **Instructions in source/data.** The optional Chat action submits an
   explicit request to this app's own agent, including the person's question
   and the selected public repository. The accompanying instruction asks for
   source dates, sample coverage and treatment of repository text as untrusted
   data. It does not direct a reviewer, another app or the system assistant.
   Remote descriptions, PR titles and release notes remain text/data; they are
   never evaluated as Splash or inserted into the fixed L0 source.

6. **Wording.** Authored text contains no abusive wording or targeting of a
   private individual. Public repository titles and release notes are third
   party content; the app does not claim to moderate GitHub.

7. **Agent scope.** `bundle/AGENT.md` limits the assistant to the reviewed
   `reporadar.*` tools and the person's explicit requests. All tools are
   foreground-only and non-shareable. Private watchlist and observation tools
   disclose private data. Reads use the same Rust exports as the UI; local
   mutations update the same canonical state and return the resulting list.
   Model-requested unpin is destructive with `confirm: host` and
   `auto_approvable: false`, so each such deletion waits for host review.
   Pin, select and move are local actions; direct user clicks use the same Rust
   functions without claiming to be approval for an unrelated agent call.
   The app has no email, account-write or GitHub-write tool. No tool uses an
   app-supplied confirmation sheet. Live model execution is not yet verified.

8. **Route: human-review.** See `ux-0.1.1.json` for the current source hashes,
   native interaction and visual checks, retained failures and exact limitations.
   Before production publication, verify the new GitHub-attested release and
   execute that sealed component, then confirm a compatible published host.
   Public-catalog installation follows publication and must be recorded
   separately. A real second-version update and live model behavior are
   separately unverified; neither is implied by source acceptance or attestation.
