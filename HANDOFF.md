# Handoff: update system, locks, host contract, SugarCubes updates, monorepo

Written 2026-11-11 for the coordinating session. Read this fully before planning.

## 0. What this is

The maintainer asked for a review of the updater and the app lock/lease system, then designed with me what the professional version looks like. This document hands all of that to you to coordinate: a board, parallel agents, verified commits.

- Everything here comes from **static reading**. No code was run and nothing has been implemented.
- The review was done on Rust `main` at `c08357e4` and SugarCubes `main` at `cb69792f`. Both move. **Re-check every file and line reference before acting on it.**
- The full evidence chains are in `D:\SugarSubstitute\review\updater-lock-review-notes.md`. This handoff is self-contained for planning; the notes have more detail per finding.
- Finding IDs (F1–F10, S3, S8, S10, S12) are stable and match the notes. Work item IDs (A1, B2, …) are new here and are meant to become board items.

Path shorthand: `ss` = `crates/sugarsubstitute`, `sup` = `crates/sugarsubstitute-supervision`, `con` = `crates/sugarsubstitute-contracts`, `desk` = `crates/sugarsubstitute-desktop`, `ui` = `crates/sugarsubstitute-ui`. `SC` = the SugarCubes checkout root.

## 1. Outcome the maintainer wants

1. Users never get stuck on an update or silently fail to update.
2. Users are never locked out by a stale lock.
3. Shutdown never loses work.
4. An update never loses work.
5. The update system is reliable and professional.
6. The lock/lease system is reliable and professional.

Plus, from the design discussion:

7. The user can always see what is happening, in a way that fits the app.
8. SugarCubes updates on its own cadence without needing a Rust release, and still feels like one system.
9. SugarCubes lives in the same repository as the Rust app (monorepo), **without configuring any remote yet**.
10. Release notes and versions come from automatic changelogs (semantic-release), like the old Python app.

## 2. Ground rules

Use the existing team rules as the base and adapt names and paths: `D:\SugarSubstitute-Rust\canvas-integration\recovery\TEAM-RULES.md`. The points that matter most:

- Read `AGENTS.md` in each repository before changing anything (monorepo root and Rust: `E:\devprojects\SugarSubstitute-Rust\AGENTS.md` and `Substitute\AGENTS.md`; SugarCubes: `Sugar\agents.md` and the nested ones under `Sugar\integrations\`).
- **Headless only.** Never launch the app, ComfyUI windows or browsers on the maintainer's desktop. Native runs use the hidden desktop; browsers run headless.
- **Commits are local. Never push. Never create or configure a remote.** No attribution trailers on commits.
- Never weaken a test, gate or budget to pass. Proof must be clean.
- New worktrees go under `E:\devprojects\`. Large outputs and evidence go under `D:\` only. Suggested evidence root: `D:\SugarSubstitute-Rust\update-system\<agent-name>\`.
- The old Python app (`E:\devprojects\SugarSubstitute`) is a reference for exactly three things, matched in behaviour, not structure: the error dialog's layout and behaviour (D8, D1w), the way a node pack is installed from the registry with a GitHub fallback (D4, F1w/F3w), and the semantic-release setup (D7, G2). Read the real Python code before building each. **Do not model locks, leases, app updating or shutdown on the Python app.** Those are designed here from the findings and decisions in this document.
- The maintainer does not want check-ins between milestones, and does not want to be asked to re-test. Agents prove fixes; the coordinator reviews evidence.
- There is **other work in progress on the Rust main checkout**. Find out what owns it before assigning anyone to edit there.
- Board: the coordinator owns the live board.
- Models (maintainer, 2026-11-11): the coordinator is Opus at high effort. Every subagent is Opus at medium effort (Agent `model: opus`, `effort: medium`). Run work in parallel as far as is feasible; the limits are checkout ownership, file overlap and the shared proof lane, described under "How the coordinator works" below.

### How the coordinator works

This is run the way the feedback-fix wrap-up was run.

- **Standing goal, no check-ins.** Hold the goal until every work item in section 5 meets section 7. Start the next item at once. Do not ask the maintainer whether to continue, and never ask them to test anything.
- **Write the rules file first.** Copy `TEAM-RULES.md` to `D:\SugarSubstitute-Rust\update-system\TEAM-RULES.md`, adapt names, paths, the board link and the Python rule above, and have every agent read it fully before working. Keep the final-report format: under 400 words, anything unfinished or unproven first, then commits, gates run with results, processes stopped, evidence paths.
- **Board.** Build it before the first agent starts, one item per work item, grouped by workstream. You own item states. An item is active only while an agent is actually working on it, and is closed the moment its work lands and its proof is reviewed. Agents post progress lines only. Never write clock times.
- **Teams own checkouts.** One agent or team per checkout; only the owner edits, stages or commits there. Each gets its own worktree under `E:\devprojects\`, its own build cache under `X:\codex-build-cache\<name>`, and its own evidence folder under `D:\SugarSubstitute-Rust\update-system\<name>\`. Two agents never write the same files at once.
- **Parallelism.** Static work, design-then-code items and unit-tested Rust work run in parallel across worktrees. Native proofs share one hidden desktop and need about 20 GiB of free RAM, so they go through a single proof lane, one at a time. Check free RAM before starting agents or heavy builds.
- **Agents prove their own work.** Each fix lands with a regression test that fails before and passes after, and real-app proof where behaviour is visible. You look at every proof image yourself before it goes on the board. A proof showing any error is not proof; the error is a bug to root-cause.
- **Verify before reporting.** Check a task's state directly before saying it is running or done. A report is a claim: read the diff and the gate output. Losses, failures and mistakes lead every message to the maintainer.
- **Usage.** Do not poll agents or read large files whole. When a usage limit stops agents, resume them with SendMessage after it resets rather than starting new ones.
- **Cleanup.** After each report, stop leftover test processes the agents started (verify each image path first). At the end, merge the work to `main` locally, and remove the worktrees this project created after preserving anything uncommitted. Never push.

Repositories:

| What | Where |
|---|---|
| Monorepo main (Rust app in `Substitute\`, SugarCubes in `Sugar\`) | `E:\devprojects\SugarSubstitute-Rust` |
| SugarCubes dev install | `E:\ComfyUI\custom_nodes\SugarCubes`, a junction to the monorepo's `Sugar\` (G1). The old standalone repository and its worktrees are at `E:\devprojects\SugarCubes-repository` |
| Old Python app (behaviour reference) | `E:\devprojects\SugarSubstitute` |
| Stale, do not use | `E:\devprojects\sugarcubes` (2025) |

## 3. How the systems work today

### 3.1 Update flow

- **Background preparation.** After the app is ready, the coordinator starts one helper process (the updater executable in its preparation role). It discovers signed metadata, downloads the package with ranged resume over HTTPS, verifies it, and stages a new version folder beside the current one. It never activates anything. One campaign lasts at most 15 minutes. (`ss/src/production/entry_runner/updates.rs`, `ss/src/updates/`, `sup/src/update_preparation/`, `sup/src/update_repository.rs`, `sup/src/update_http.rs`)
- **Activation at next launch.** The stable launcher is headless. It re-verifies the staged candidate from cache with no network, starts it as a **trial**, and falls back to the accepted version on any optional failure. Its only time limit is 3600 seconds. (`ss/src/stable_launcher/lifetime*.rs`, `con/src/coordinator_entry/deadline.rs:17`)
- **Trial.** The launcher takes the run lock, records "trial" in the selection record, and passes the lock handle to the new app. The app accepts the trial once main window, browser and ComfyUI backend are all ready. If the app closes or dies first, the trial is **rejected**, and a rejected package is blocked from every automatic retry. Only the installer (repair intent) or a newer release gets past that. (`sup/src/setup_storage/selection/run*.rs`, `con/src/installation/transition.rs`, `ss/src/production/coordinator_authority/installation/`)
- **Installer.** The installer executable embeds the app package and a compiled-in copy of the signed metadata. (`sup/src/setup_storage/signed_installer*.rs`, `sup/src/setup_storage/download/local.rs`)
- Versions are side by side under `releases\<id>`. User data is outside them. The accepted version only changes on trial acceptance.

### 3.2 Locks

All of these are operating-system locks that the kernel releases when the process dies. There are no PID files and no age-based stealing.

| Lock | Where | Kind | Held |
|---|---|---|---|
| Run lock | `selection\.installation-run-lock` | File opened with no sharing | Coordinator lifetime, plus the custody helper during a trial |
| Selection writer | `selection\.selection-lock` | Byte-range, non-blocking try | Every read and write of the selection record |
| Trust writer | `update-trust\writer` | Byte-range, non-blocking try | Trust reads and writes, **including whole payload copy and verify** |
| Preparation | `update-trust\preparation-v1` | Byte-range, non-blocking try | One updater campaign |
| Single-instance election | `coordinators\<ns>.entry.claim`, `.claim`, `.record` | Byte-range + 128-byte record | Coordinator lifetime |
| WebView profile | per profile | Byte-range | Coordinator lifetime |

Child processes are created with explicit handle lists and kill-on-close jobs, so ComfyUI cannot inherit a lock. Watchdogs exist: the parent challenges the UI process, and the custody helper can terminate an unresponsive parent.

### 3.3 Work persistence

Workspace checkpoints autosave after 250 ms quiet, at most 2 s behind, published atomically with the previous copy kept. Close takes a final capture inside the shutdown budget (10 s graceful, 15 s total). (`crates/sugarsubstitute-application/src/workspace_session/schedule.rs:17`, `ss/src/workspace_session/`)

### 3.4 Host contract (SugarCubes side)

- Contracts live in `SC\integrations\contracts`: `panel.schema.json`, `canvas.schema.json`, `masks.schema.json`, `session.schema.json`, each with generated TypeScript and Rust. Shared client/server code is in `SC\integrations\shared`.
- Hosts today: Substitute (native bridge), Photoshop (UXP plugin, `SC\integrations\photoshop`), GIMP (companion app + plugin, `SC\integrations\gimp`), and plain ComfyUI (no host). More hosts are planned.
- The host **declares** presentation: it sends a `configure` message with visual style, palette, accent and resize-corner size. SugarCubes replies applied or rejected.
- The canvas has capability discovery: the page asks, the host lists what it implements, absent means unsupported, and the host replies in the schema the page asked for.
- The Rust app vendors copies of these schemas under `third_party\sugarcubes-*` and registers them in `governance\persistence\catalog.toml` as exact-version, reject-on-mismatch.
- A fifth protocol, memory (`sugarcubes.memory`), is defined in Rust (`con/src/memory.rs`) and in `SC\frontend\comfyui\ui\core\memory\`, not in the contracts folder.

## 4. Decisions already made by the maintainer

Do not reopen these. Where something is marked "recommended, not confirmed", ask once if it blocks you.

**Update behaviour**

- D1. A prepared update stays valid offline. Record a local receipt for the exact prepared payload when preparation completes under valid signed authority. The launcher refreshes metadata when it can and honours a revocation it sees.
- D2. **During beta, updates are applied without asking**, because the maintainer needs users on the latest release. It must fail gracefully, leave the app usable, and produce a log the user can hand over. Build the normal ask/notify policy now as well; graduating from beta is a policy switch, not new code.
- D3. A SugarCubes update never restarts the ComfyUI backend mid-session. Show that an update is available and let the user decide when to restart.
- D4. SugarCubes is updated from the Comfy Registry first, then from GitHub when the registry lags, installed so the result is **identical to a registry install**. This is how the old Python app did it. Do not rely on the registry alone.
- D5. Metadata lifetimes (delegated to me): timestamp 14 days, re-signed daily by scheduled CI; snapshot and targets 90 days, re-signed at each release and monthly; root 1 year with rotation. This depends on D1 and on fixing F10.

**User interface**

- D6. The "update ready" control is at the top right of the window, a button that looks like minimise/maximise/close.
- D7. Release notes shown in the app come from the automatic GitHub changelog. Releases and changelogs are set up like the old Python app: CI plus semantic-release.
- D8. An update failure is reported through the app's error modal, following the Python one. Building that system is in scope if Rust lacks it, and so is giving the existing crash and error modals the Fluent look.
- D9. Presentation stays as named styles (comfy, fluent, spectrum, photopro, gimp). Move the PhotoPro palette out of the fields every host must send.

**Architecture**

- D10. SugarCubes owns the host contract. SugarCubes must always work on its own, outside Substitute.
- D11. SugarCubes and the Rust app release independently from one monorepo. A web release must never require a native release: it uses whatever the installed host declares and degrades gracefully. "Half-working" is acceptable if it is graceful. The thing that must be correct is the canvas talking to the web side and back.
- D12. Native releases are mostly about canvas capabilities. Publishing order (native, web, web, native, web…) is a discipline: a web release may use what the preceding native release added. It is not an install gate. A native release that breaks the contract ships paired with a web release.
- D13. Move SugarCubes into the monorepo now. Do not configure a remote for it yet. The monorepo should be able to publish SugarCubes to the Comfy Registry and to GitHub later.

- D14. The whole project is licensed under the AGPL. This replaces the GPL-3.0-or-later headers and licence metadata on the Rust side (`AGENTS.md` currently requires GPL-3.0-or-later SPDX headers on first-party Rust source; that rule, the headers, `Cargo.toml` licence fields and `third_party` notices change together, in one dedicated commit). Third-party code keeps its own licence.

- D15. Layout: the node pack itself lives in the top-level folder `Sugar\` (not `Sugar\SugarCubes\`), and the Rust host with the canvas lives in `Substitute\`. See G1.
- D16. **SugarCubes keeps its name and its existing registry entry; the original pack is updated** (decided 2026-11-11). Nothing is renamed: registry id, installed folder name, protocol names, setting ids and saved-workflow names all stay `SugarCubes` / `sugarcubes`. Only the monorepo folder is called `Sugar\`. A renamed pack as a new registry entry was considered and dropped. See G4.

- D17. **Launch is the update point for everything.** Beta "without asking" means update at launch: the app update is checked, downloaded and applied before the app opens, with the splash showing progress, and the current version starts if anything fails (confirmed by the maintainer).
- D18. **In Substitute, the host installs a SugarCubes update during the splash**, before the ComfyUI backend is started, so no mid-session restart is ever needed (consistent with D3). When SugarCubes is not hosted by Substitute it updates itself. An earlier recommendation in this document (SugarCubes downloads its own update in every host and Substitute only restarts) is withdrawn.
- D19. The host contract files stay where they are inside the pack (`integrations\contracts`, which becomes `Sugar\integrations\contracts`). They are not lifted into a separate folder as part of this work. If a host outside this repository ever needs them, publish them as a versioned package from that folder; do not move them.

The maintainer delegated the next three to my recommendation ("as you recommend", 2026-11-11). Treat them as decided. They touch update security, so the coordinator reviews the implementation closely, but does not need to go back to the maintainer.

- D20. **Update policy lives in the build.** The policy value (beta-eager or standard) is compiled into each build's distribution configuration, next to the repository identity and feed (`ss/src/updates.rs:20`, the embedded `update-distribution.json`). It is not remotely switchable and is not a field in signed metadata. Graduating from beta means releasing a build whose policy is standard; beta builds update eagerly at launch, so that build reaches everyone.
- D21. **Trust writer hold (A2).** Three steps: (1) authenticate the offer under a short writer hold, as today; (2) stage and verify the payload under the preparation lock only, with no trust writer held; (3) take the writer briefly, re-read current trust and release history, run the same verification again against them, then authorise the release history and publish the selection record in that one hold. The final check and the commit stay atomic under the writer, which is the guarantee the current code protects. A rotation or subscription change between steps 1 and 3 fails step 3 and nothing is published.
- D22. **Receipt for a prepared update (C3, D1).** When preparation completes, and in the same writer hold as D21 step 3, write a protected local receipt binding: installation identity, generation id and directory identity, package digest, release sequence and channel, and the trusted root version at that moment. At launch, a candidate may activate on the receipt plus its verified bytes when cached metadata is expired or missing, provided all of these hold: the release sequence is not below the installation's release floor (rollback protection unchanged); the trusted root version has not advanced since the receipt was written (a root rotation invalidates receipts and requires an online refresh); no newer metadata seen since then omits or supersedes that target (revocation); and the receipt is no more than 90 days old. When any condition fails, the launcher starts the accepted version and the next online check re-prepares. Model the record on `InstallationAuthorization` (`sup/src/setup_storage/selection/completion.rs`).

## 5. Work

Each item gives the problem, the evidence, what done looks like, and how to prove it. Items marked **investigate** have an unknown to settle first.

### Workstream A — Locks and contention

**A1. Short locks wait instead of failing (F3, F4).**
- Problem: the selection writer and the trust writer use a non-blocking try and treat contention as a hard error almost everywhere. Only the launcher's two selection tasks retry.
- Evidence: `sup/src/setup_storage/selection.rs:268` (`writer`), `sup/src/setup_storage/trust.rs:204-209`, retry only at `ss/src/stable_launcher/lifetime/contention.rs:30`. Hard-failing callers include `selection/run.rs:75` (`InstallationRun::acquire`), `selection/reservation.rs:72`, `selection/run/launch.rs:99`, and every `replace`/`compare_exchange`.
- Done: one bounded wait-with-deadline inside each lock owner, used by every caller. No caller writes its own retry. Lifetime locks (run, election, profile) keep failing fast.
- Prove: tests that hold the lock from a second handle for a short time and show every public operation succeeds after it is released, and fails with a typed timeout if it is not.

**A2. The trust writer covers only read-and-commit (F4).**
- Problem: `publish_evidence` holds the trust writer across payload staging and verification, which can take a long time.
- Evidence: `sup/src/setup_storage/trust/activation.rs:156-203`; the long work is inside the publish closure at `sup/src/setup_storage/transaction/candidate.rs:127-211` and `transaction/reservation.rs:31-93`.
- Done: long work runs under the preparation lock, and trust is re-validated at commit. This is security-sensitive: the existing comment says the writer is retained "so rotation cannot race the final check". The design is decided in D21: authenticate under a short hold, stage and verify under the preparation lock only, then re-verify and commit in one short hold.
- Prove: a concurrent trust reader succeeds while a payload copy is in progress; a key rotation between verify and commit is detected.

**A3. Record pins do not block replacement longer than needed.**
- Problem: a launcher's retained read handle on the selection record makes a concurrent record replacement fail with a storage error.
- Evidence: `selection.rs:220` (`read_pinned`), `selection/reservation.rs:29-33` (`ReservedApplication._selection`).
- Done: replacement waits out a short pin (A1's wait applies), or pins use a separate snapshot as trial forwarding already does (`selection/reservation/forwarding.rs`).

**A4. Run lock uses a byte-range lock (S3, investigate).**
- Problem: the run lock is a file opened with no sharing. Any other program opening that file (antivirus, indexer, backup) could make the app believe another copy owns the installation.
- Evidence: `sup/src/durable_storage/protection.rs:188-194`, classification at `selection/run.rs:156-169`.
- Investigate first: confirm the failure by having a test hold an ordinary shared handle on the lock file.
- Done: same exclusion mechanism as the election and profile locks, including across the trial handle transfer (`selection/run/transfer/custody.rs`), with `verify_exclusive_rendezvous` updated to match.

**A5. An unreadable single-instance record self-heals when no owner is alive (F5).**
- Problem: an undecodable or wrong-length coordinator record fails every launch, permanently, even when this process holds both election locks.
- Evidence: `sup/src/coordinator_entry/native.rs:375`, `sup/src/durable_storage/protected/record_file/native/read.rs:94`, `ss/src/production/entry_bootstrap/failure.rs:24`.
- Done: with both exclusions held, move the unreadable record aside (preserve the bytes), log it, and proceed as a recovered election. Keep fail-closed when the coordinator exclusion cannot be taken.
- Prove: fixtures with garbage, truncated and wrong-namespace records, with and without a live owner.

**A6. "Already running" is always actionable (S8, investigate).**
- Investigate: what a second launch shows when the first copy is alive but unresponsive: the forwarding deadline, the message, and how the watchdogs (`ss/src/production/parent_lifecycle/watchdog.rs`, `crates/sugarsubstitute-execution/src/windows_process.rs:44`) bound it.
- Done: a second launch brings the existing window forward within a second or two, or shows the Close / Force surface (restyled under D1w). No path ends with nothing on screen.

### Workstream B — Trial and acceptance

**B1. Three trial verdicts (F2).** Highest-value fix.
- Problem: a trial that ends without acceptance is always rejected and the release is quarantined. Causes that are not the release's fault: the user closes the window during a slow first start, ComfyUI fails for its own reasons, Windows shuts down, a power cut, or B2.
- Evidence: reject at close `sup/src/setup_storage/selection/run/settlement.rs:25`, called from `ss/src/production/coordinator_authority/installation/settlement.rs:19`; reject after crash `selection/run/recovery.rs:17`; quarantine `con/src/installation/transition.rs:46,103`; accept only after full readiness `installation/health.rs:17`.
- Done: three outcomes.
  - Healthy: accept.
  - Bad: the new host itself crashed or failed its own startup. Roll back and count a strike.
  - No verdict: everything else. Return the candidate to "prepared" and try again next launch. The record transition for this already exists (`cancel_unstarted_trial`, `transition.rs:120`); it needs a sibling that is valid after a process was created.
  - Quarantine takes more than one strike and expires. The persisted record format changes, so register the migration in `governance/persistence/catalog.toml`.
  - Check quarantine **before** download, not after import (`transaction/reservation.rs:60`, `transaction/candidate.rs:156`).
- Prove: fault-injection tests for each cause above; a quarantined release is not downloaded again; a no-verdict trial activates on the next launch.

**B2. Acceptance cannot hurt a healthy app (F3).**
- Problem: the acceptance write is one-shot. If it fails once (for example lock contention from a second launch), the coordinator closes the freshly updated app, and B1's old behaviour then quarantines the release.
- Evidence: one-shot `ss/src/production/coordinator_authority/installation/publication.rs:29`; failure closes the app `coordinator_authority/completion.rs:20` → `coordinator_authority.rs:255-268`.
- Done: acceptance retries until it succeeds or the app closes. A failure that never resolves is a no-verdict, and the app stays open.

### Workstream C — Updater behaviour

**C1. Scheduled checks with retry (F1).**
- Problem: one attempt per app run. `update_attempted` is set once and never cleared; the first error ends the campaign; there is no retry anywhere.
- Evidence: `ss/src/production/entry_runner/updates.rs:11-17`, `ss/src/updates/parent.rs:97-103`, `ss/src/updates/session.rs:114-126`.
- Done: check shortly after startup, then every few hours while open. A failed check retries with backoff. Connection drops resume within the attempt. No overall time cap on a download, only a stall timeout. All scheduling goes through the execution owner (see `AGENTS.md`, Execution ownership).

**C2. Update policy: beta-eager and standard (D2, R2).**
- Done: a policy value selects between (a) update at launch without asking and (b) prepare silently, notify, apply on next launch or on "restart to apply". Both are built and tested now. The policy value is compiled into the build's distribution configuration (D20); it is not remotely switchable.
- In beta mode a failure at launch starts the current version, shows one line, and files an update report (D4w).

**C3. Offline-valid prepared updates (F7, D1).**
- Problem: activation re-authenticates saved metadata against the wall clock. Expiry is the minimum over all four signed roles. A user whose sessions are further apart than the timestamp lifetime never activates.
- Evidence: `crates/sugarsubstitute-update/src/trust.rs:103-114,192`, `sup/src/setup_storage/trust/offer_cache.rs:64`. The installer path already has an expiry-independent authorisation to model on: `sup/src/setup_storage/selection/completion.rs` (`InstallationAuthorization`).
- Done: per D1, with the receipt contents and the four activation conditions in D22.
- Prove: tests for each condition failing on its own (release below the floor, root rotated, target revoked in newer metadata, receipt older than 90 days), and for activation succeeding offline with expired cached metadata.

**C4. A downloaded installer does not expire (F10).**
- Problem: the installer checks its compiled-in metadata against the clock. After the shortest-lived role expires, that installer file refuses to install.
- Evidence: `sup/src/setup_storage/signed_installer.rs:98-111`, `sup/src/setup_storage/download/local.rs:90`.
- Done: if embedded metadata is expired, refresh it online; if offline, install the exact embedded payload (root-signed, digest compiled in, explicit user choice) and let the updater bring it current.

**C5. The launcher is never invisible (F6).**
- Problem: no window, one-hour bound, 100 ms re-poll of lock contention up to that bound, and after a reported cleanup cutoff the headless loop can wait forever.
- Evidence: `ss/src/stable_launcher/lifetime/contention.rs:16`, `ss/src/stable_launcher/lifetime.rs:262`, `desk/src/headless/state.rs:105`.
- Done: normal launches show nothing extra. After about two seconds a small "Starting…" surface appears (reuse the splash). Waits on another process are bounded in seconds and fall back to starting the accepted version. The launcher process always exits.

**C6. Old versions are cleaned up (F8, investigate).**
- Investigate: confirm nothing removes `releases\<id>` folders. I searched `sup/src/setup_storage` only.
- Done: keep accepted, previous (for rollback) and at most one candidate. Remove the rest once nothing holds them, under verified installation ownership (`AGENTS.md`, Persistence and upgrade safety).

**C7. Download robustness (S10, investigate).** Disk-full handling and retention of partial downloads in `sup/src/setup_storage/download*` and `cache/*` were not read.

**C8. Metadata lifetimes and re-signing (D5).** Release tooling for the signed repository was not found in this repo. It needs to exist, with the lifetimes in D5 and a scheduled re-sign job. Part of workstream G.

### Workstream D — What the user sees

**D1w. One native report surface with the Fluent look (D8).**
- Today there are three unrelated surfaces, none Fluent: the crash report window built from raw Windows controls (`desk/src/report/windows/controls.rs:82-103`), startup failure via `MessageBoxW` (`desk/src/startup_error_dialog.rs:31`), and "another copy is running" via `TaskDialogIndirect` (`desk/src/startup_conflict.rs:83`).
- Python reference to read first: `E:\devprojects\SugarSubstitute\sugarsubstitute_shared\presentation\error_report_view.py` (layout: severity glyph, title, message, summary, collapsible animated details with the full report text, footer Copy / Report issue / Open console / Close, Restart as primary when offered), `error_report_dialog.py`, `error_report_window.py`, and the presenter `substitute\presentation\errors\error_presenter.py` (non-blocking, de-duplicates repeats).
- Done: one report view in `sugarsubstitute-ui` and one report model with severity and kind. Four callers: crash (keeps its dump checkbox and Export), startup failure, another-copy-running (keeps Close, then Force after 15 s), update failure (new). Register toolkit and platform sources in `governance/ui/policy.toml`. Modals are never entered from an event callback. All text goes through the localization owner.
- Not checked: whether SugarCubes has its own in-app error modal that this should match.

**D2w. Update status contract and controls (D6).**
- Done: a new protocol beside the existing ones (suggested name `sugarcubes.updates`), in the SugarCubes contracts folder with generated bindings. The host reports state (current version, channel, last check, available, ready, failed) and accepts "check now" and "restart to apply". It is optional: hosts that have no updater never speak it.
- Native: an update button in the caption area beside minimise/maximise/close (`ui/src/caption.rs`, `desk/src/windows_caption.rs`), with ready and warning states. It works when the web interface is unavailable.
- In SugarCubes: an Updates entry (current version, channel, last successful check, "Check for updates" with live result), shown only when the host offers the protocol.
- Words: "ready", "up to date", "couldn't check", "previous version is running". Never "trial", "candidate", "quarantine".
- Restart to apply runs the normal close with its final workspace save, then relaunches. If a workflow is executing, offer "after this finishes".

**D3w. Startup messages.** The splash shows "Updating to version X…" when applying. First start on a new version shows one dismissible "Updated to version X" with a link to the changelog (D7). After a rollback, one plain line that the previous version is running.

**D4w. Update report (D2, D8).** Each failed update attempt produces a readable report (version, phase, cause chain, no secrets or unnecessary paths). The update button shows a warning state; clicking opens the report surface with Copy and Export and no Restart. Export only; nothing is uploaded.

**D5w. Windows shutdown and logoff (F9).**
- Problem: no handling of session end anywhere in `crates/` (window loop is tao 0.37). The app is killed without its close flow.
- Done: answer the session-end query, register a short block reason ("Saving your workspace"), run the same final capture as a normal close, exit in the time Windows allows. An interrupted trial is a no-verdict (B1).

**D6w. Close keeps everything authored (S12, investigate).** Canvas and input-document persistence at close and across an update was not traced (`ss/src/input_documents`, `ss/src/native_canvas`). Confirm that everything the user authored is durable before the window disappears, or that the app says what was not saved.

### Workstream E — Host contract

**E1. Unknown discovery schemas step down instead of switching off.** First thing to do in this stream; it is the one blocker for D11.
- Evidence: `SC\integrations\shared\CanvasSurfaceDiscovery.ts:98-123,171` answers "unsupported" with empty capabilities for anything but schema 2 or 3. Rust special-cases schema 1 the same way at `con/src/canvas_surface/capabilities.rs:142-150`; behaviour for a schema above 3 was not checked.
- Done: a host asked for a schema it does not know replies with the newest it knows; the page steps down. Both directions tested.

**E2. A host description as the front door.**
- Problem: which protocols a host speaks is inferred from which bridge was found. Three host facts travel as properties on the injected bridge object, outside any schema: `preferencePolicy`, `outputSurfacePreference`, `memoryBudget` (hardcoded true for any native transport). Evidence: `SC\frontend\comfyui\ui\core\embedded\NativePanelTransport.ts:64-84`, `BrowserPanelTransport.ts:22-32`.
- Done: one declared description listing the protocols a host offers and those three facts. Add it as a new protocol or section so existing hosts keep working (existing schemas are strict: no additional properties, version fixed at 3).

**E3. Remove host-identity checks.** `isSubstituteHost` is used at `SC\frontend\comfyui\ui\surface\appearance\AccentThemeHost.ts:85` (really "host offers a system accent") and `SC\frontend\comfyui\ui\output_canvas\infrastructure\ComfyCanvasHost.ts:191` (really "host requires its native output surface"). Replace each with a declared capability. The contracts folder already states the rule: capabilities are declared independently of editor identity.

**E4. Memory schema joins the others** in `SC\integrations\contracts`, generated and tested the same way.

**E5. Presentation (D9).** Keep named styles. Make the PhotoPro palette a non-required field. This is a panel schema change that touches every host; plan it with E2 so hosts change once.

**E6. Conformance kit and host profiles.** The contract schemas plus tests a host runs against itself (discovery, each capability, step-down). Each host release records its discovery reply as a fixture; SugarCubes CI replays all recorded fixtures. Build the kit so a host in another repository can run it.

**E7. Rules to hold in review.** The envelope stays fixed and new features are capabilities. A capability is used only when declared. Changed behaviour gets a new capability name. New protocols are added beside existing ones.

### Workstream F — SugarCubes updates

**F1w. Rust setup installs SugarCubes in registry format.**
- Problem: setup copies a `sugarcubes` folder out of the app package into `custom_nodes\SugarCubes` only when the folder is absent, as a plain copy with no ownership file, and nothing refreshes it. Evidence: `sup/src/setup_storage/transaction.rs:176-190`; no `.tracking` or registry references anywhere in `crates/`.
- Python reference: `E:\devprojects\SugarSubstitute\substitute\infrastructure\comfy\` — `nodepack_registry_installer.py` (exact version from the registry), `pinned_nodepack_source.py` (`install_fallback` from a pinned GitHub tag archive; `_apply_tracked_source_transaction` replaces only files listed in `.tracking`, backs up, rolls back, and refuses a folder without `.tracking`), `nodepack_registry_tracking.py` (`.tracking` is a newline list of owned relative paths), `nodepack_manifest.py` (SugarCubes entry and pins).
- Done: the installed folder is indistinguishable from a registry install, so both the app and ComfyUI Manager recognise it.

**F3w. Substitute installs SugarCubes updates during the splash (D18).**
- Where: after the app itself is current and running its splash, before the managed ComfyUI backend starts. The pack's files are not loaded by anything at that moment.
- How: the same acquisition as F1w (registry first, GitHub release fallback, registry format with `.tracking`), replacing only tracked files with a backup, as the Python app did at startup.
- Health and rollback: if the backend then fails to come up with the pack, or the page does not establish its session, restore the backup and start again on the previous pack version. Use the three verdicts from B1: only a failure of the new pack itself counts against it.
- Failure never blocks startup: if the check or download fails, start with the installed pack, show one line, and file an update report (D4w).
- While the app is open: a newer pack is only reported as available (D3). It is installed at the next launch, or when the user chooses "restart to apply".
- Attached ComfyUI that the user runs themselves: the backend may already be running with the pack loaded, so the host must not replace files under it. Report the update and leave it to SugarCubes' own updater (F2w) or the next time the host starts that backend.
- Decide and state where this lives in Rust and how it shares test vectors with the Python implementation in F2w, since there are now two implementations of the same install format.

**F2w. SugarCubes updates itself when not hosted by Substitute: registry first, GitHub fallback (D4, D18).**
- SugarCubes already has this acquisition for node packs: `SC\sugarcubes\backend\services\dependency_registry_source.py`, `dependency_source_archive.py`, `dependency_source_git.py`.
- Done: outside Substitute, SugarCubes checks for and stages its own update, applies it only when the user chooses, and tells the user to restart ComfyUI. Inside Substitute it does not self-update; it defers to the host (F3w) and learns that from the host description (E2).
- Ownership: only touch a folder with a valid `.tracking` file. A git checkout or a folder with local edits is left alone, and the Updates entry says so.
- For an attached ComfyUI that the user runs themselves, the host cannot restart the backend: stage and apply the next time the host sees that backend start, or tell the user.

### Workstream G — Monorepo and releases

**G1. Move SugarCubes into the monorepo (D13).**
- Do not configure or push to any remote.
- Preserve the history of SugarCubes `main`. The layout is decided (D15).
- **Decided (maintainer):** the other SugarCubes branches are not carried into the monorepo. Only `main` moves. Do not inventory, merge or port the roughly 45 worktrees and their branches. This is not permission to delete them or the existing repository; leave both where they are.
- ComfyUI must still find SugarCubes at `custom_nodes\SugarCubes` for development. Plan for that (for example a directory link) and for every script, rule file and memory that names the old path, including `TEAM-RULES.md`.
- The Rust repo's vendored schema copies under `third_party\sugarcubes-*` become references to the single source; update `third_party/manifest.toml` and `governance/persistence/catalog.toml` in the same change.
- Licence: see D14 (whole project AGPL).
- Constraints any layout must meet:
  - The node pack must be one self-contained folder (`Sugar\`) with `__init__.py` and `pyproject.toml` at its root, because ComfyUI loads a custom node from exactly that shape and the registry publishes it. Its installed folder name under `custom_nodes` stays `SugarCubes` (D16).
  - The Rust workspace root (`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo`, `crates`, `tools`, `governance`, `third_party`) moves as one unit so every path inside it stays the same relative to that root. `AGENTS.md` says to run commands from the repository root; that sentence and CI working directories change to the workspace folder.
  - `SC\integrations\gimp\companion` is a second Rust project with its own toolchain pin. It stays a separate workspace.
- **Decided (maintainer):** the monorepo publishes per-component tags and releases of the node pack, and the updater's GitHub fallback consumes those. The note below is only the implementation detail that follows from it.
- A GitHub tag archive of a monorepo is the whole repository, not a SugarCubes-rooted folder. The Python fallback (`install_fallback`, single-root zip of the SugarCubes repo) and SugarCubes' own dependency code (`archive_urls`: `<repo>/archive/refs/tags/v<version>.zip`) both assume the repository root is the node pack. In the monorepo the GitHub fallback (D4) must download a SugarCubes release asset built by CI, in registry layout, not the tag source archive. Tags also need a per-component format; `v<version>` alone no longer identifies SugarCubes.
- Anyone who installs SugarCubes by cloning its git repository into `custom_nodes` gets a folder that is not a node pack once the source lives in a monorepo. Registry installs are unaffected. If git installs must keep working, the old SugarCubes repository has to be kept as a published split of the `SugarCubes` folder. That is a remote decision for later, but the layout should not make it harder.

**G4. Pack identity (D16, decided).**

SugarCubes keeps its name and its existing registry entry, and existing users receive the monorepo's releases as ordinary updates. Nothing is renamed. Only the monorepo folder is called `Sugar\`. A product display name can change later without touching any identifier.

- The installed folder is still `custom_nodes\SugarCubes` while the monorepo folder is `Sugar\`. The development link and the registry package both map one to the other.
- Version numbers continue the existing SugarCubes sequence. Per-component tags in the monorepo start from the current SugarCubes version, not from zero.
- **This project does not touch the existing SugarCubes remote git repository.** No pushes, no tags, no notices, no changes to the registry entry's repository link. The entry and the pack's metadata name that repository, and git-clone installs and tag-archive downloads read from it. Whether the entry later points at the monorepo, or that repository keeps receiving tags, is a remote decision for later. Do not act on it.
- Workflows saved by any earlier SugarCubes must keep opening. The names saved inside workflows are a frozen file format, verified at SugarCubes `cb69792f`:
  - Node type names (`SC\sugarcubes\nodes.py:232-238`): `SugarCubes.CubeInput`, `SugarCubes.CubeOutput`, `SugarCubes.ManagedImageOutput`, `SugarCubes.OutputSavingPolicyError`, `SugarCubes.SpatialProbeMeasurement`; the frontend also refers to `SugarCubes.Cube`.
  - Keys written into each node's `properties` and each subgraph definition's `extra`, such as `sugarcubes_kind`, `sugarcubes_cube`, `sugarcubes_document`, `sugarcubes_symbol`, `sugarcubes_surface`, `sugarcubes_output_saving`, `sugarcubes_widget_values`, `sugarcubes_cube_version`, `sugarcubes_pack_name`. About forty `sugarcubes_*` names appear in the frontend source; some are runtime-only markers that are never saved.
- **Substitute is unreleased. The only installation anywhere is the maintainer's own machine.** Do not build compatibility layers or data migrations for Substitute-side formats on behalf of users who do not exist (for example the selection-record change in B1, or host-contract changes in workstream E, where every host is in this repository and changes together). Where the repository rules require a registered migration, do the minimum the gate needs and say so. SugarCubes itself does have users: their saved workflows and ComfyUI settings must survive an update.

Considered and dropped: releasing a renamed pack ("SugarUI" or "Sugar") as a new registry entry. It would have split users across two entries and required a collision guard, a settings migration and a permanently frozen set of old names.

**G2. Releases and changelogs (D7).**
- Python reference: `E:\devprojects\SugarSubstitute\.releaserc.cjs` and `.github\workflows\release*.yml` (semantic-release: features and breaking changes bump minor, fixes and perf bump patch; generated notes; `CHANGELOG.md`; version files committed back; GitHub release with assets). SugarCubes has the same shape in `SC\.releaserc.json` and `SC\.github\workflows\release.yml`. The Rust repo has only `verify.yml`.
- Done: per-component versions, tags and changelogs from one repository, with path-filtered releases so a SugarCubes-only change releases SugarCubes only. The in-app "what changed" link reads the generated changelog. Workflows are written and tested locally; nothing is pushed.
- SugarCubes publication targets, for later: Comfy Registry and GitHub releases, same artifact bytes.

**G3. Signed repository tooling (C8, D5).** Produce and re-sign the update metadata with the lifetimes in D5.

## 6. Order and parallel lanes

Dependencies:

- A1 before B2 (B2 relies on waiting locks). B1 and B2 land together.
- C3 and C4 before the metadata lifetimes in decision D5 can be relied on.
- D1w (report surface) before D4w and the restyle of the conflict surface in A6.
- E1 before any independent web release. E2 before E3 and E5.
- G1 before G2. F1w and F2w are easier after G1, since the contract and release tooling move.
- D2w's protocol is defined in SugarCubes and consumed in Rust: define the schema first, then both sides in parallel.

Suggested lanes, chosen so no two agents edit the same files:

| Lane | Owns | Items |
|---|---|---|
| Storage and locks | `sup/src/setup_storage/selection*`, `trust*`, `durable_storage/protection*`, `coordinator_entry*` | A1–A5, B1 record changes, C3, C4 |
| Coordinator and launcher | `ss/src/production/coordinator_authority`, `entry_runner`, `ss/src/stable_launcher`, `ss/src/updates` | B1 host side, B2, C1, C2, C5, C6 |
| Native UI | `ui`, `desk/src/report`, `desk/src/startup_*`, caption | D1w, D2w native half, D3w, D5w |
| SugarCubes contract and UI | `SC\integrations`, `SC\frontend` | E1–E6, D2w web half |
| SugarCubes backend | `SC\sugarcubes\backend` | F2w |
| Repository and release | repository layout, CI, release tooling | G1–G3, F1w packaging side |

Start with: A1, B1+B2, E1, and the G1 inventory and layout proposal. They are independent and remove the cases where a good release is thrown away.

Keep the agent count low. Native proofs share one hidden desktop and need about 20 GiB free RAM.

## 7. How we know it is done

- A release that reaches a machine is running there within two launches, or the user has been told why not.
- No sequence of double-clicks, early closes, sleeps, shutdowns or power cuts loses a release or blocks a launch.
- Every wait longer than two seconds has something on screen.
- Disk use for app versions is flat over time.
- A newer SugarCubes on an older host, and an older SugarCubes on a newer host, both run, with unavailable canvas features switched off cleanly.
- Each statement above is a headless test with fault injection at every write, not a claim.

## 8. Not covered by the review

Do not assume these are fine.

- ComfyUI backend resource leases and the custody helper internals (`sup/src/native_claims`, `sup/src/registry`, `sup/src/lifecycle_protocol`).
- Entry forwarding transport in `crates/sugarsubstitute-execution`.
- `crates/sugarsubstitute-application/src/supervision/desktop_exit*`.
- The installer and setup flow beyond the files cited.
- `crates/sugarsubstitute-update` beyond `trust.rs`.
- The Photoshop and GIMP host implementations beyond their presentation code.
- Tests were not read.

## 9. Questions to put to the maintainer when they block you

1. Anything in "Not covered by the review" that turns out to change a decision above.
