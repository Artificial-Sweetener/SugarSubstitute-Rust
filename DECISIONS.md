# Coordinator design decisions

Every design call the coordinator makes for an agent, with its reasoning. Maintainer decisions D1–D22 live in HANDOFF.md and are not repeated here.

Each entry answers: root cause, what the constraint protects, options compared on correctness, the choice, and the test that proves it.

## C-1. A1 lock wait (storage lane)
- Root cause: pure reads take the selection/trust writer lock, so a second launch reading the record collides with the updated app's acceptance write (F3).
- Constraint: only the execution owner may wait natively, so waits are owned, cancellable and torn down.
- Options: (a) try_lock polling via an execution pause port; first approved, then withdrawn as a minimum fix; (b) kernel wait.
- Choice: pure reads become lock-free snapshot reads opened with FILE_SHARE_DELETE, which also fixes A3. Writer-vs-writer on Windows uses an execution-owned overlapped LockFileEx wait raced with cancellation and the deadline, with CancelIoEx and completion drained before returning. Non-Windows uses a bounded backoff on try_lock through the execution clock, because those platforms have no timed lock primitive. Typed Timeout and Cancelled. No caller retries.
- Proof: a held writer does not block a reader; a reader holding the file does not block replacement; a waiter wakes promptly on release, times out at the deadline, and cancels with no outstanding I/O.

## C-2. A4 run lock and updater preparation lock (queued, storage lane)
- Root cause: share-mode-0 opens mean any foreign reader (indexer, antivirus) blocks the lock.
- Constraint: exclusion must survive the cross-process trial handoff. Windows byte-range locks are per process and file object, so a naive swap breaks the handoff.
- Choice: gated byte-range slots with an acquire-before-release handoff (inv-storage FINDINGS.md), applied to both transferred locks. The other share-0 rendezvous move to rendezvous() plus try_lock().
- Proof: a foreign shared reader no longer blocks acquisition; a second acquire still fails; native cross-process qualification shows the parent can exit while the child keeps exclusion.

## C-3. C6 old-version retirement (queued, storage lane)
- Choice: a retirement owner in setup_storage, running under run custody plus preparation. A tombstone rename acts as the restart-safe journal. Only inventoried files are deleted. Unknown content is preserved. Keep accepted, previous, one candidate and the stable-entry source.

## C-4. C7 disk-full (queued, coord lane after C2)
- Choice: a typed InsufficientSpace with a free-space preflight before download and before staging. A shortage reclaims through cache maintenance and C6 retirement first. Logical budget exhaustion gets its own variant. Stage files become transient so they cannot leak. C1 backoff treats it as non-transient, and it is reported through D4w.

## C-5. D1w report surface (ui lane)
- Choice: one report model and view in sugarsubstitute-ui, hosted by the reviewed Fluent prepared-pixels form window the installer uses (one host, not two). Frame preparation runs in bounded slices on the UI thread, because these surfaces must work when the execution runtime has failed. Export writes go through a caller-supplied port. The Save dialog follows the modal-owner rules.
- Proof: the installer behaves exactly as before; the window stays responsive while details are prepared; a native hidden-desktop capture of all four kinds.

## C-6. A6 second launch against an unresponsive copy (queued after D1w)
- Root cause: the forwarder only forwards, with no deadline on what the user sees. The coordinator replies Accepted after ShowWindowAsync even when the UI thread is hung, and it has no foreground rights.
- Choice: a typed, versioned entry outcome (Accepted / Unresponsive / Closing). The client grants foreground rights to the verified peer pid. The coordinator probes the window's responsiveness with an owned job (SendMessageTimeout with abort-if-hung). The forwarding process shows the D1w conflict surface after a 2 s event-driven deadline, targeting the instance. An issued launch whose peer is lost re-elects. Watchdog suspicion feeds reactivation. The hour-long silent forward is removed; 3600 s stays only as a backstop.
- Proof: a silent server shows the surface at 2 s on the manual clock; a hung probe gives Unresponsive; peer-lost re-elects; native frozen-UI qualification; the window comes forward from behind another window.

## C-7. D5w session end (queued after D6w and D1w)
- Root cause: tao 0.37 calls process::exit(0) on WM_ENDSESSION in its hidden thread-target window, in every tao process, and never surfaces WM_QUERYENDSESSION.
- Options: (a) subclass the main window and tao's thread-target window (by class name), with a qualification test that fails if tao changes the class or behaviour; (b) patch or fork tao to expose a session-end event, which is vendoring and needs third-party registration and ongoing fork maintenance; (c) upstream a hook, which is not available now.
- Choice: (a) now, with a pinned-version test guarding the internals we rely on; propose (c) upstream later. On query: ShutdownBlockReasonCreate on the main HWND, post a typed SessionEnding, return FALSE while the close-save owner (C-8) has work outstanding, never block in the window procedure. WM_ENDSESSION(TRUE) never calls exit. The coordinator records an interrupted trial as no verdict (B1). The headless loop gets the same interception.

## C-8. D6w close keeps authored work (persist lane)
- Root cause: two independent final saves; the input save runs after shutdown is requested; failures only log; non-file raster bases are not archived.
- Choice: one close-save owner gating the exit intent, with a typed CloseSaveOutcome; a visible "Saving your workspace…" state; a not-saved record plus a notice at the next start; input saved at stroke end with a 2 s longest delay; non-file raster bases archived content-addressed if the test confirms they are lost. Restart-to-apply and session end use the same owner.

## C-9. E1 discovery step-down (sc-contract now, Rust half later)
- Choice: negotiated schema = min(requested, newest known) on both answering sides. The request has a minimum of 2 and no small maximum. The page accepts 2 up to the requested schema and gates features by the negotiated schema. The Rust host stops rejecting 4+ at decode.

## C-10. Lock-free record reads (storage lane, refines C-1)
- Evidence (storage): on NTFS, a reader opened with share-delete still makes classic MoveFileExW(REPLACE_EXISTING) fail; SetFileInformationByHandle(FileRenameInfoEx, REPLACE_IF_EXISTS|POSIX_SEMANTICS) succeeds and the reader keeps the old bytes.
- Choice: record publication uses the POSIX-semantics replace; every record reader, pins included, opens with share-delete. A pin whose identity changed re-resolves the selection from the new record instead of failing startup.
- Conditions: ReFS (Dev Drive) must be verified to work, not quietly fail closed; unsupported volumes (FAT/exFAT) give a typed, actionable error at install or setup time, never mid-update.
- SUPERSEDED: ReFS support for POSIX-semantics rename could not be proven (no ReFS volume; creating one needs admin; Microsoft documentation names no file systems). Replaced by C-11.

## C-11. Reader-writer record lock (storage lane, replaces C-10)
- Choice: pure reads take a SHARED byte-range lock on the existing writer lock file for the read only; writers take it EXCLUSIVE; both wait through the LockWait port (overlapped LockFileEx on Windows, bounded backoff elsewhere). Classic replace stays; it works on NTFS, ReFS and FAT/exFAT. Long-held pins are replaced by a value identity (record sequence or digest) re-checked under the lock at the point of use. A changed accepted generation gives a typed Superseded for the launcher to relaunch; other changes re-resolve inside the owner.
- Why over C-10: no dependence on unproven file-system behaviour, testable here, no excluded volumes, and no reader can ever block replacement.

## C-12. G1 monorepo execution (repo lane)
- Choice: the Rust repository becomes the monorepo in place, so E:\devprojects\SugarSubstitute-Rust stays the main checkout and existing worktrees, caches and memory stay valid. Histories are joined by prefix moves plus an unrelated-histories merge, not filter-repo, so in-flight SugarCubes branches keep their ancestry. In-flight branches merge with `-X subtree=<prefix>`; plain rename detection was tested and misplaces new directories. Contracts become single-source through a governed xtask component root with rejection fixtures. The SugarCubes extension root becomes link-safe before any link exists.
- AGPL (D14) moves to the last commit, after every lane has merged, so one sweep also covers files the lanes add.
- Waiting on the maintainer: the dev link means moving the SugarCubes main checkout out of E:\ComfyUI\custom_nodes, and the handoff says to leave the existing repository where it is.

## C-13. C2 helper result and progress (coord lane)
- Root cause: the preparation helper reports only exit 0 or failure, so its parent cannot tell "prepared" from "up to date" and has no cause chain or progress.
- Options: exit codes (cannot carry causes); a result file (no live progress); running the campaign inside the launcher (loses containment and the engine's independent self-update, and enlarges the stable launcher); a typed pipe channel.
- Choice: keep the helper process for both background and launch-time preparation, with one typed, bounded, versioned anonymous-pipe channel in its explicit handle list (progress plus final result), built on the execution crate's existing pipe machinery. The state comes from the selection record via the storage owner; the channel reports the attempt; exit codes are a coarse class only. Execution edits wait until storage's port commit lands.

## C-14. Dev link (maintainer answer, 2026-11-11)
- Maintainer: "Back up sugarcubes into devprojects and then we will link or install or whatever into e:/comfyui from the monorepo."
- Plan: after G1 merges to main, with ComfyUI stopped: back up .sugarcubes (cube library) and .env to D: with a SHA-256 manifest; move the SugarCubes checkout, intact, to E:\devprojects\SugarCubes-repository (this is the backup into devprojects; nothing deleted); git worktree repair and verify its linked worktrees; carry .sugarcubes and .env into <mono>\Sugar\ and verify hashes; junction E:\ComfyUI\custom_nodes\SugarCubes -> E:\devprojects\SugarSubstitute-Rust\Sugar; headless ComfyUI load proof. Done after the sc-contract lane has committed, so worktree repair cannot disturb live work.

## C-15. Update-attempt channel mechanics (coord lane, refines C-13)
- Choice: no parallel copy of the diagnostic pipe. Its mechanics (reservation, bootstrap and enrollment, client bind, bounded frames, retirement) are extracted into one owned purpose-typed channel in sugarsubstitute-execution, used by both the diagnostic and update-attempt purposes, with the old code removed. A separate owner is kept only for any part whose lifecycle genuinely differs. coord also owns sup/src/registry* for carrying the inherited handle. Execution edits wait for storage's port commit.

## C-16. E1 review (sc-contract)
- Accepted: the request schema minimum stays 1, so a schema-1 request still gets an explicit unsupported reply instead of no reply.
- Required: an invalid reply fails discovery at once with a typed failure and clean feature shutdown, not a 30 s timeout. Chromium Playwright runs from a D: browsers path; only Firefox stays a known gap, and any remaining failure is proven pre-existing on unmodified main.

## C-17. SugarCubes gate failures exposed by E1 (sc-contract)
- Found: the tauri_shell suite launches Firefox for every test, so its Chromium variants fail without Firefox; the frame-allocation browser suite times out page creation under full-suite load (it passes alone).
- Choice: fix both at their root (each variant launches only its own browser; find and remove the load contention; no hook timeout increase). Firefox variants remain the maintainer-approved known gap and are never skipped or deleted. The invalid-reply fix commits after these.

## C-18. Report window sizing (ui lane)
- Root cause of the hung proof: tao's set_inner_size adds the hidden frame of an undecorated shadowed window, so the client was larger than requested, and the host presents only a frame matching the window, so nothing was ever shown.
- Choice: the report composes at the window's real size, acknowledged by the host each turn; the host corrects the offset; native tests fail at once with sizes when no matching frame exists and no work is pending.
- Changed: a window too short for the report is never an error. The window is clamped to the monitor work area and the message and summary scroll, with title and buttons visible, so the report shows on a 1366x768 laptop at 150% scaling.

## M-1. Maintainer direction (2026-11-11): another copy, and plain words
- Maintainer: "Instead of giving the user the option to cancel a run or force quit only, what about requesting a graceful exit/lock clear, showing progress on that, and then starting the app when it's clear?" and "not comfortable with how much of lease/lock we make the user think about. The copy is very technical."
- Behaviour:
  - A live, responsive copy gets no dialog: its window comes forward.
  - A copy still closing is waited for automatically, with a splash progress line ("Finishing closing the previous window…") after about 2 s, and this launch continues by itself once it exits. Only Cancel is offered.
  - Only a copy that has stopped responding, detected by a responsiveness probe rather than a timer, gets a plain question: "Close it and start" or "Cancel".
- Words: no user-facing text says profile, lock, lease, instance, election, trial, candidate or quarantine, and none shows an install or profile path outside the collapsible report. Every message says what is happening and what the user can do.

## C-19. Lock wait shape (storage lane, refines C-1 and C-11)
- The infrastructure gate rejects a handle in a contracts signature, so the wait is the execution-owned module `sugarsubstitute_execution::file_lock::lock(&JobContext, BorrowedHandle, Shared|Exclusive, until)`, registered as a native_custody boundary. It uses an overlapped LockFileEx; a registered wait races the injected-clock deadline and cancellation; on a non-grant it calls CancelIoEx and waits for completion, unlocking if the lock was granted in the race. It refuses non-blocking jobs and synchronous handles.
- No non-Windows path: the lock owners (setup_storage) are Windows-only, so it would have no caller.
- A5 committed: 1f640b82 (update/storage).

## C-20. Storage gate false positive on the Export dialog (ui lane)
- The gate's native access list matches every identifier starting with "SetFile", so IFileDialog::SetFileName and SetFileTypes count as raw filesystem access.
- Choice: fix the gate precisely instead of weakening Export. Exact IFileDialog configuration method names are excluded, with rejection fixtures proving SetFileInformationByHandle, SetFilePointerEx, SetFileAttributesW, SetFileTime and SetFileValidData are still caught. It lands as its own fix(xtask) commit with `cargo xtask verify`.

## C-21. SugarCubes browser-suite setup budget (sc-contract, revises C-17 item 2)
- Finding: no contention inside the frame-allocation suite. Its setup hooks use Jest's 5 s default, while other browser suites give setup 30–60 s, and the failure reproduces only under machine-wide CPU starvation from parallel agent builds.
- Choice: one shared setup-hook budget, the value the other browser suites already use, exported from browser test support and used by every browser suite. Test bodies and assertions are unchanged. The invalid-reply fix commits on its latest gate, whose only failures are Firefox variants. The Tauri shell fix is 700f4b38.

## C-22. D6w follow-ups (persist lane)
- ad4888b5 lands the close-save owner: input save before shutdown, Stale on cutoff, not-saved record and notice, and a 250 ms / 2 s input autosave.
- WITHDRAWN: the reported sync hang on main was wrong. It came from persist's own uncommitted adapter reserving 32 GiB, which execution answers with MemorySaturated, and input jobs retry that forever.
- Real defect behind it: admission answers MemorySaturated even when a reservation can never fit. persist adds a typed permanent AdmissionError::ExceedsCapacity in admission_capacity.rs, and every MemorySaturated-retrying caller treats it as permanent.
- Then content-addressed stored bases for non-file sources, with reference-counted cleanup. The record's spelling source joins the catalog fingerprint.
- Open for later lanes: a `StartupWindowContent` hook delegating to `StartupHost::request_close(CloseReason)` for D2w restart-to-apply and D5w session end.


## C-23. Ownership-verified retirement as a public storage API
- Stored input-canvas bases and old app generations (C6) both need ownership-verified deletion. Storage exposes one public retire API built on durable_storage/retirement.rs as part of C6: ownership verified, reparse points refused, unknown files preserved, restart-safe. persist lands stored bases with pruning decided now and executes it once the API exists.

## C-24. D1w review (ui -> ui2)
- Accepted: the Fluent report view and the startup-failure, unresponsive-window and update-failure surfaces (images reviewed clean, plain wording per M-1); the xtask dialog fix.
- Rejected before merge: the adapter recognised the conflict controller's state by matching its English sentence. ConflictView gets a typed phase, and the controller moves to the execution clock.
- Remaining for D1w: crash surface migration, with a proper form-host phase between window retirement and content settlement.

## C-25. Purpose-pipe buffer loans (coord2)
- Found: a test hung because it held a rejected write buffer while waiting for the session to end, and checked retirement waits on every outstanding loan.
- Choice: make the trap impossible by construction instead of a "return the buffer before close" rule. Loans return themselves when dropped, or closing takes ownership of or invalidates outstanding loans; retirement wakes when a loan is dropped. A test proves a caller holding a rejected buffer can close and settle.

## C-26. D6w landing and D5w assignment
- Accepted: 4b3e628d (typed permanent ExceedsCapacity), e23bcb48 (record spellings under the fingerprint), aa6bd36a (content-addressed stored originals for temp and output bases). The cleanup decision is implemented and tested; executing it waits for storage's retire API (C-23).
- Required: an ExceedsCapacity test for workspace decode, encode and publish, with the fixture made capacity-configurable.
- D5w goes to persist: a desktop adapter subclassing the main window and tao's thread-target window, with a block reason, typed SessionEnding into request_close, and never exit(0); a qualification test guards tao's internals.

## C-27. Selection record schema 2 and cross-version protocols (storage -> storage2)
- Accepted: d6ee6aa9, which adds `read_update_selection`, a `SignedRelease` on `Generation`, and `rollback` on the record (schema 1 -> 2, no migration; the maintainer's own install needs a reinstall once this reaches a build they run).
- Not accepted as-is: the updater probe and the trial/launch transfer protocols changed wire shape without a version change. The stable launcher can be older than the generation it starts, so these are cross-version protocols. storage2 bumps and registers them, and proves both skew directions fall back safely or refuse with a typed error.
- storage2 continues with A4, C6 (with the public retirement API and the disk-shortage reclaim hook), then B1's record side.

## C-28. D2w web half and updates protocol (sc-contract)
- Accepted: a92c047e. It adds `sugarcubes.updates` v1 (watch, check, and restart now or after the workflow; a state with a revision, status, report, previousVersionRunning, workflowRunning and scheduledRestart; every request answered accepted or rejected), the SugarCubes Updates entry, and the `UpdateSource` seam for F2w (`applies: comfyui-restart`, attached only when the host does not serve updates).
- Added: an `unmanaged` reason `linked-folder` for junctions and links. F2w's frontend source is a new file owned by sc-backend.
- E6 next: a portable conformance kit plus recorded host profiles replayed in CI; the Substitute profile slot stays pending until Rust records it.

## C-29. D5w window subclassing (persist)
- persist proposed subclassing only tao's thread-target window, which is the one that calls exit(0).
- Choice: subclass both the thread-target window and the visible main window, answering from one owner state. The visible window, which holds the "Saving your workspace" block reason, answers FALSE while the close save is outstanding, so Windows' shutdown screen names the app and its reason instead of a hidden, nameless window. Small edits in desktop startup_runtime.rs, startup_events.rs, native_action.rs and host_contract.rs are approved, coordinated with ui2's restart-to-apply hook on the same trait.

## C-30. D1w crash surface (ui2)
- Accepted: 16d5e6d2 (the unresponsive-window question uses a typed phase in every language; the execution clock) and f46776c2 (crash report in the Fluent window; the old raw-control window deleted; Export uses the shell Save dialog with an overwrite prompt; the form host gains window retirement before settlement). Images reviewed clean.
- Wording per M-1: "Include a memory snapshot (helps us find the cause)"; "A memory snapshot can contain passwords or other private information."; "Restart SugarSubstitute"; a message line that says what to do next.
- Gap: Export through the real shell dialog is covered only by unit tests with a fake session; native harnesses cannot drive the shell dialog.
- The restart-to-apply delegation in StartupHost goes to persist, alongside its session-end delegation.

## C-31. Update report and update button while the app is open (ui2, coord2, persist)
- The form host gets a second mode: an owned top-level window on the running app's event loop (owned by and centred on the main window, the same report view, no second event loop). The standalone mode stays for startup failure and crash. Chosen over a child surface because it works when the web interface is down and avoids WebView z-order problems.
- Restart to apply: ui2 defines `request_restart_to_apply` on StartupWindowContent; persist delegates it in StartupHost to request_close(RestartToApply).
- "After this finishes": coord2 owns a typed workflow-activity source from the managed backend's own status stream. Restart runs when the queue empties, and an unreachable backend while waiting fails visibly.

## C-32. A4 trial handoff acknowledgement (storage2)
- Found: Windows byte-range locks belong to (process, file object), and closing any handle releases them. The launcher exits right after creating the trial app, so slots need an acknowledgement or there is a gap where a second launch can take the run lock.
- Choice: trial entry v4 carries a "slot held" event. The creator keeps TrialTransfer until the event fires or the child exits, through an execution-owned wait raced with the deadline and cancellation. The receiver takes its slot, then requires the sender's handoff byte to be still held, otherwise it refuses. Failure paths: child exit means no verdict and the accepted version starts; at the deadline the sender releases, the child refuses, and the accepted version starts. storage2 owns the whole path, including the broker wire and the stable_launcher creator.
- Protection::exclusive_rendezvous itself moves to rendezvous() plus try_lock(), so other owners' share-0 users move without editing their files.
- C-25 refined (coord2): two existing owners deliberately use buffer loans to order retirement (the recovery supervisor's Stop frame before its Retired write; diagnostic reporting's family closure), so a global "never wait" would break them. The pipe core now fixes a per-session rule at reservation: update-attempt sessions never wait on caller-held loans (each held guard keeps its own memory charge and frees its storage on drop); diagnostic, desktop, custody and recovery sessions keep their deliberate fence. Tests cover both rules.

## C-33. E6 conformance kit (sc-contract)
- Accepted: f77c5697 (four unmanaged reasons, exact restart wording) and b8881d98 (a portable `runHostConformance` kit, an in-memory host channel, and recorded profiles for gimp, photoshop and comfyui replayed against current page code).
- Pending marker: Jest `test.failing` for the Substitute profile is allowed. It is visible and temporary, and goes away when the Rust host records its profile.
- Gap: the GIMP and Photoshop canvas answers are recorded from a shared CanvasServer configured like their bridges, not from the native bridges, which need native APIs. Presentation and description come from the real editor code.
- Next: masks in the kit; E7 as executable contract tests (frozen envelopes per version, declared-capability use only, new names for changed behaviour).

## C-34. F2w self-update (sc-backend)
- Accepted in design: 8b917128 (registry-first checks with a GitHub release-asset fallback; journaled tracked-file replacement in prestartup; rollback on failure, power cut or an unconfirmed start; pack_install as the single owner of the install format) and 55964c7e (the Updates entry fed through the UpdateSource seam). Shared vectors: tests/pack_install/support/registry_install_vectors.json.
- External finding: every SugarCubes registry version from 0.12.0 to 0.15.3 is marked Flagged on the Comfy Registry. Like the Python app, updates install only from Active versions, so updates currently come only through the GitHub fallback. This is for the maintainer.
- G2 must publish: a release tagged sugarcubes-v<version>; an asset SugarCubes-<version>.zip in flat registry layout, byte-identical to the registry publish, uploaded so GitHub records its sha256; the repository named in release_source.json.
- Required before merge: an owned digest manifest so local-edit detection works offline; loopback transport tests; a headless real-ComfyUI proof of apply and rollback; installed_pack_folder unified with the link-safe extension root after G1.

## M-2. Registry flags (maintainer, 2026-11-11)
- Maintainer: "we shouldn't care if the registry flags it". The SugarCubes updater treats Flagged registry versions exactly like Active; it still skips Deleted, Banned and any other non-Active, non-Flagged status. Registry first, GitHub fallback only when the registry lags.
- Maintainer, same day: "It also doesn't even mean there isn't a newer version." Every check queries both the registry's full version list and the GitHub releases, and takes the newest across both. It installs from the registry when the registry has that exact version, otherwise from the GitHub asset. It is "up to date" only when neither source is newer. If one source fails, it uses the other and reports which one couldn't be checked.

## C-35. Cross-version protocol versions (storage2, closes C-27's condition)
- Accepted: 846a5f10. Accepted launch is protocol v3, trial launch v4 and the updater probe v3. Each side compares declared numbers before creating a process; skew in either direction is refused before trial, and the accepted release starts. Record decode reads schema_version first and gives a typed Rejection::Schema.
- Consequence for later releases: the stable launcher is replaced only when the new launcher keeps the previous accepted-launch protocol and reads the current selection schema. Both are effectively frozen across updates, so a future change needs a launcher that speaks both.
- The maintainer's install needs a fresh install once this reaches a build they run (no migration; unreleased).
- Flaky test registry::tests::cleanup_wakeups::watched_live_root_receives_one_step_per_fact_across_repeated_driver_turns failed once under load; assigned to coord2 to make deterministic.

## C-36. Thin stable launcher (proposed, storage2 mapping first)
- Problem (from C-35): the root launcher must speak the accepted-launch protocol and read the selection schema, so both freeze across every future update, or each change needs N/N-1 compatibility code in the launcher.
- Target: the root SugarSubstitute.exe reads only a tiny, permanently stable pointer (current generation plus a fallback), starts that generation's own launch engine, and exits. Record parsing, trial and rollback, and every handoff protocol live in the generation and ship with the app they serve, so they can change freely. The frozen surface is only the pointer and "start the engine". If the engine fails to start, the fallback generation starts.
- Next: storage2 maps the current root-versus-generation split and recommends sequencing relative to A4. The maintainer was told and may weigh in.
- C-36 approved after storage2's map. The root today is a full copy of the latest launcher running the whole state machine, with only update-engine activation delegated. Order: A4 part one; then the thin launcher (pointer record and frozen root reader, an engine launch role with fallback, engine-side app creation, broker removed); then A4 part two inside the engine; then C6. The root verifies only the engine bytes against the protected pointer's hash and size. Cross-generation rules: an engine reads the previous and its own schema, writes a newer schema only after acceptance, and the fallback starts its app without parsing the record. The pointer is a permanent, additive-only format. Root waits are bounded in seconds. Fault injection at every step must end in a running app or a visible report. B1 and B2 move to a new agent `trial` with a file split.

## C-37. B1 three verdicts and B2 (trial)
- The record's `rejected` becomes `strikes` per package (count, last time). Quarantine needs 2 strikes within 7 days and expires; accepting clears strikes and rollback. Selection schema 2 -> 3, no migration.
- Transitions: `reject` = Bad (rollback, +1 strike); `release_trial` = NoVerdict after a process existed (back to prepared). An unwitnessed ending (power cut, kill, a host crash before the witness enrolled) is NoVerdict, but 3 consecutive unwitnessed endings make one strike, so an early-crashing release cannot loop forever.
- The verdict comes from a pure typed classifier over observed facts (readiness reached, close requested, which process failed), never from timing. On SessionEnding the trial settles as NoVerdict at once, before the close save, so a forced kill after Windows' time limit never counts as Bad.
- Quarantine is checked before download through `Admission::Withheld`. B2: acceptance and settlement retry through the short-lock owner until success or close; any failure leaves the app open and records no verdict.
- Approved out-of-lane edits: update_preparation/state.rs, backend_custody installation/worker/retirement, the transfer/witness finish_settled signature (early commit, storage2 informed), and an observe_host_fault call where UI or main readiness fails.
- C-36 details confirmed (storage2):
  - The root is a copy of sugarsubstitute-launcher.exe. With no arguments it takes the thin path: verify `<gen>\sugarsubstitute-launcher.exe` against the pointer on a pinned handle, then start `--launch-engine-v1 <installation> <took-over event>`. The frozen surface is the pointer v1, that spelling and the event.
  - The root waits 10 s for took-over or engine exit, terminates a silent engine it created, then tries the fallback; if the fallback also fails, it shows the startup report and exits non-zero.
  - Pointer: primary and fallback, written at preparation (primary = candidate) and at the verdict; the trial runs entirely in the candidate's own generation.
  - The selection record and trust files become forward-tolerant: additive fields are ignored by older readers, and the schema number bumps only for incompatible changes, so a fallback after antivirus removes the new engine still works. Only after an incompatible bump does the fallback refuse, with a plain "reinstall; your files and settings are kept" report.
  - The broker is deleted; the newer-engine case folds into the pointer.
- C-36 refinements:
  - storage2 A4 part one: 5e2c0581 (frozen slot exclusion native-exclusion-slots-v1; the preparation lock on slots with handoff adoption; exclusive_rendezvous as rendezvous() plus try_lock()).
  - Self-healing redirect: each engine checks the record, and if another generation should run, rewrites the pointer and starts that engine with a hop count. Only the installer and redirects write the pointer.
  - `fall_back_to_retained` (assigned to trial): after a newly accepted engine goes missing or corrupt, the retained release is accepted again and the newer one is marked for repair (no strike, no quarantine). The next check re-prepares it. The user sees "SugarSubstitute is running the previous version while it repairs the update."

## M-3. No wall-clock pass/fail in tests or gates (maintainer, 2026-11-11)
- Maintainer: "why do we keep adding fixed 60 second tests and other bullshit like that". New team rule: no test or check passes or fails on elapsed seconds. Use deterministic clocks or observed outcomes, with no-progress hang guards. Agent `timeouts` inventories existing cases in both repositories and proposes enforcing gates; each case is then fixed by its owner.

## C-38. D5w proof (persist)
- Accepted: 22e1853f plus the native proof session-end-07. The main window answered FALSE with "Saving your workspace…", the workspace saved, no close-save record was written, and the app exited by itself without the tao exit(0) path.
- Gap: WM_ENDSESSION arriving while the save is still running is proven only by the unit test with a real hidden Tao loop, because the save finished in under 0.5 s in the native run.
- The coordinator half (an interrupted trial records no verdict) comes with B1 (trial, C-37).

## C-39. F2w proof and acceptance point (sc-backend)
- Accepted: 642b056a (both sources every check, Flagged equals Active), 8dbf29d3 (offline digest record beside .tracking), 8f7f4d44 (a trickling download can no longer outlast its deadline; at most 5 redirects; plain HTTP only to loopback), 7a87e550 (test release-source configuration). The headless real-ComfyUI proof passed: install, upgrade, a broken version, and rollback with a plain notice.
- Proof side effect fixed: the first run created an empty E:\ComfyUI\user\comfyui.db.lock. The coordinator removed it; later runs pass --database-url under D:.
- Required: a version is accepted only after a full ComfyUI start (every SugarCubes node class registered and the server-started hook reached), and a frontend-reported load failure on first page load also triggers rollback.
- Also found: two 0-byte model files in the maintainer's model library. SugarCubes logged an unnamed traceback for them; sc-contract is turning that into one plain, named warning.
- M-3 inventory (timeouts): Rust has 12 (a) sites, SugarCubes about 120, plus Playwright's implicit defaults. This includes the coordinator's own 60 s shared setup budgets, which become far hang guards. Assigned: sc-contract (TS/Jest sites and gate 2), sc-backend (pytest sites and gate 3), rs-timeouts (Rust sites, the xtask real-time gate 1, and the CI timeout). Inventory: timeouts\INVENTORY.md.

## C-40. D2w native half (ui3)
- Accepted: 9eaf4613. Caption peers: the update button (ready arrow green, warning amber), the restart offer, one message line (saving or not saved), a reserved link, and Dismiss. The update report opens as a HostedForm owned by and centred on the main window, on the app's loop. Pull methods on StartupWindowContent: update_status, close_save_progress, take_unsaved_notice, workflow_activity, update_report_export, locale_messages. Images reviewed clean.
- Next: merge to main, then persist implements the StartupHost side (close-save, notice, messages) and coord2 feeds update_status, workflow_activity and report export. Real-app proof after that.

## C-41. Update-attempt channel (coord2)
- Accepted in design: 66342615 (purpose-typed pipe), 6a69d876 (loan rule per session), cb6ed611 (helper progress and result to the starting app), bebd2b36 (registry tests deterministic under load; 80-run stress clean). update/coord stays unmerged pending av-hunt's access-violation finding.
- Required: PREPARATION_PROTOCOL bumped and catalog-registered. The updater engine can come from another generation than the app that starts it, so skew in both directions refuses with a typed error and shows as an update failure. "Unreleased" waives data migrations, not cross-version protocol versioning.

## C-42. Access violation investigation (av-hunt)
- Not reproduced in more than 120 runs of the exact binary under a first-chance AV catcher (31 full, 40 in-flight subset, 44 GPU, 5 main-era). No faulting stack. The branch is ruled out by code: the new loan rule applies only to UpdateAttempt sessions, which the app crate never creates, and the sugarsubstitute lib has no unsafe code. update/coord is released for merge once coord2's protocol bump lands.
- Harness kept: av-hunt\harness\Run-Caught.ps1 dumps and symbolizes any recurrence.
- Real pre-existing flakes found: a viewport settlement race (a CPU repair using Arc::get_mut while a display job holds a clone) and a temporary-outputs deadlock report, both going to av-hunt on update/canvas-flakes; the autosave-window test (persist's own) goes to persist under M-3.

## C-43. Update status across the coordinator and the desktop child (coord2)
- The lifecycle protocol (coord2 owns sup/lifecycle_protocol* and production/child/channel*) gains typed, bounded messages, with a version bump registered. Coordinator to child: an UpdateStatus snapshot with a revision. Child to coordinator: RestartToApply, sent only after close_save_outcome() is Some. The coordinator relaunches the root SugarSubstitute.exe exactly once, only on that intent plus a normal child exit.
- Workflow activity: a coordinator-side ComfyUI websocket client (option A) on queue_remaining, with bounded reconnect, working for managed and attached backends and without the web interface. Losing the backend while waiting fails visibly and never restarts.
- Update report export runs in the child from the snapshot.

## C-44. D3w startup messages (ui3)
- Accepted: 0c68a2c7. The splash shows "Updating to version X…" (image reviewed clean). The first start on a new version shows "Updated to version X" with a "What's new" link (https only). After a rollback, one line: "The previous version is running." Inputs are take_update_notice and StartupStatus::Updating, which coord2 feeds; the changelog URL is a typed distribution-config input until G2.

## C-45. G1 layout ready (repo)
- Accepted: monorepo/layout at c032cf6c. Commits 1–7 are done; SugarCubes history is imported at d8e38f04, and later SugarCubes commits come in via -X subtree=Sugar merges. Contracts come from a single source through governance/monorepo/components.toml with xtask rejection tests. The Rust host answers discovery for schemas above 3 in schema 3 (E1's Rust half). G4 identity was checked by blob ids (pyproject, __init__, nodes.py, prestartup, package.json, frontend/, web/).
- Landing: once main is green, repo merges current Rust and SugarCubes mains into the layout and gates it, then the integrator merges it to main and all lanes switch to the monorepo layout. Dev link (commit 8) and AGPL (commit 9) follow.
- C-39 follow-up (sc-backend): 35433f84 (accept only after ComfyUI is listening with all nodes registered; the backup is held until the first page load; a page-reported failure rolls back on the next start), b3392cb0 and 59b9e519 (pytest waits on observed events, a shared 600 s no-progress guard, an injected clock for the transport deadline), and 81fdef83 (the Python AST gate, zero violations). The headless proof passed, including a late import failure and a page failure. Required: the server-start wrap must fail safe when ComfyUI changes. A missing hook falls back to node registration plus page load with one warning, and never rolls back a good version.
- C-41/C-43 follow-up (coord2): 008ae5e9 bumps PREPARATION_PROTOCOL to 2, refused on skew both ways (catalog native-update-preparation-invocation-v2). Item 2 may edit production/parent_lifecycle/**. The ComfyUI queue websocket is split: a bounded loopback websocket transport job in sugarsubstitute-execution (cancellable, masked frames, pings, oversize reported), with the ComfyUI protocol, queue_remaining decoding and reconnect in sugarsubstitute-comfy. New dependency: the RustCrypto `sha1` 0.10 crate (default features off) for the RFC 6455 Sec-WebSocket-Accept check, from the same maintainers as sha2 and digest already in the lock; no hand-rolled hash.

## C-46. B1 landing review (trial)
- Accepted in design: 09ea12ab (schema 3 with strikes, release_trial, fall_back_to_retained, the verdict classifier, Withheld before download, B2 retries).
- Fix required: a UI child exiting before readiness with no close request and no session end is Bad, not no verdict; otherwise a release whose window crashes at startup retries forever.
- trial also wires the SessionEnding no-verdict settlement (it may edit desk/headless.rs) and removes the duplicated watchdog exit literal (execution uses the contracts constant). coord2 shows the repair notice through take_update_notice.

## C-47. Keeping the two SugarCubes installers from drifting (for F1w/F3w)
- The Python updater (F2w) and the Rust installer (F1w/F3w) share one language-neutral vector file, `Sugar/tests/pack_install/support/registry_install_vectors.json`, read in place by both test suites after the monorepo lands.
- It is extended to cover decisions as well as layout: source selection (registry vs GitHub, flagged, one source down), rollback triggers, and ownership refusals.
- A repository check fails unless both suites consume the file and every vector is exercised by both.
- The .tracking and digest-record formats carry a version in the shared catalog.
- C-46 follow-up: trial 5612a8d5 (a UI window ending on its own before readiness is a strike; SessionEnding settles no verdict at once; desk/headless.rs gains a HeadlessLifetime::session_ending hook). Schema 3 is an incompatible bump; the record ignores unknown fields except in Trial. Remaining before merge: carry the SessionEnding close reason with the child's exit, so a session-end close is never a strike whatever the order.

## C-48. Throughput changes (maintainer: "I had hoped work would move a bit more rapidly")
- The integrator merges each reviewed branch as soon as its `cargo xtask check` passes on the merged tree, and runs `cargo xtask verify` on the accumulated tip in parallel with later merges. A verify failure is fixed forward, never by rerunning to green.
- Known pre-existing canvas flakes (av-hunt's list) are tracked and fixed in parallel; they don't block merges. "Main green" still means a full verify with no unexplained failure.
- Agents decide routine matters themselves and note them in their report: test-only or one-line signature edits outside their lane, fingerprint refreshes and fixture updates. Design choices and ownership conflicts still come to the coordinator.
- C-48 refinement: full verifies run from a dedicated detached worktree, E:\devprojects\SugarSubstitute-verify (its own cache), so merges into the main checkout never disturb them. The monorepo layout lands as soon as it contains d50439de and its xtask check and npm check pass; verify follows in the verify worktree.

## M-4. Combined verification (maintainer, 2026-11-12)
- The maintainer asked whether lanes should be merged and tested together, and said a failure "should still be fixed by the integrator anyway… we don't send the code back to the agent that wrote it".
- Lane commit gate: `cargo xtask check` plus the tests of the modules the change touches and the tests it adds. Full crate suites and `cargo xtask verify` run once, on the combined main, in the verify worktree. This overrides AGENTS.md's per-commit crate-suite rule for this project.
- The integrator owns every failure found on the combined code: it narrows it to a commit and fixes it. A failure goes back to a lane only when the fix is a design change inside that lane's area. If the integrator falls behind, the coordinator adds a fixer agent.
- Real-app proofs are grouped by feature on the combined main: the update flow (coord2, ui3, trial, storage2) and SugarCubes installs (packinstall, host).
- Build slots: every wrapper takes one of three shared named-mutex slots (`Local\SugarSubstituteBuildSlot0..2`) before running cargo, with CARGO_BUILD_JOBS=10 inside a slot.
- M-4 adjustments (coordinator, under the maintainer's go-ahead to adjust strategy):
  - Lanes run their whole subsystem area's tests at commit, not just the touched modules; it catches self-inflicted breakage early at a cost of minutes, not a build.
  - The integrator merges and verifies. When a verify on combined main fails (other than av-hunt's known canvas flakes), it reports to the coordinator, who starts a dedicated fixer agent with that failure. The fixer reads the lane's notes and the decision log, may ask the original lane, and fixes with a regression test on its own branch for the integrator to merge.
  - No hard cap on agents: the shared build slots throttle CPU. Lanes off the critical path wait for slots like everyone else.

## C-49. Thin launcher landed on its branch (storage2)
- Accepted: b4c999eb (execution handle_wait), e3ad505b (thin stable entry: pointer read, engine hash pinned, 10 s bound, fallback, always exits), 02e234b9 (activation broker removed; a newer updater takes over through the engine role), 52ceed87 (fallback to the previous release when an engine is damaged; plain reinstall text when needed). Fault injection covers corrupt or missing pointer, changed engine bytes, early exit, silent engine, missing fallback, routing and reinstall.
- Follow-ups: Trial also forward-tolerant (or written in the previous schema until acceptance) so a fallback during a trial starts the accepted release; a typed pointer-generations function for retire's keep set. Native qualification is approved. C6 now belongs to retire.

## C-50. Rust pack install (packinstall)
- Accepted: 42ed5e0f. The shared vectors gain formats (.tracking v1, digest record v1), rendered digest bytes, and sourceSelection, updateRefusals and rollbackTriggers groups, driven through the real Python updater; a contract test fails if a group has no driver.
- The Rust module lives in supervision `pack_install/`; F3w's backup and journal live in Substitute's own storage; format versions sit in the catalog under a shared monorepo component.
- Zip: the `zip` crate (default features off, deflate via flate2), with our own per-entry validation (no absolute or `..` paths, no links, size bounds, regular files and directories only), each refusal tested. Not a hand-written parser.
