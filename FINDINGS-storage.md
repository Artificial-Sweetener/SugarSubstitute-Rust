# inv-storage findings (A4/S3, C6/F8, C7/S10)

Static reading at Rust `80073ca6`. Saved by the coordinator from the agent's report (the harness refused the subagent's own write). `sup` = `crates/sugarsubstitute-supervision/src`, `ss` = `crates/sugarsubstitute/src`.

## Settle first
1. Windows `LockFileEx` locks belong to the locking process + file object and are released when that process closes the handle or exits, even if a duplicate lives in another process. A naive swap of the run lock to a byte-range lock breaks the trial handoff (launcher -> trial app -> custody helper). Needs acquire-before-release; confirm with a cross-process native test.
2. Handoff 3.2 is wrong about the preparation lock: it is share-mode-0 `updater-preparation-v1`, transferred across processes and checked by `verify_exclusive_rendezvous` (`sup/setup_storage/trust/updater/custody.rs:6,17,35`). A4 covers two transferred locks.
3. User-visible text traced only to the generic `show_startup_failure` modal.

## A4 / S3 — CONFIRMED by reading
- `Protection::exclusive_rendezvous` (`sup/durable_storage/protection.rs:188-194`) = `CreateFileW(GENERIC_READ|GENERIC_WRITE, share=0, OPEN_ALWAYS)` (`:240-257`). Any foreign handle with read-data access -> `ERROR_SHARING_VIOLATION` -> `classify_run_open` (`selection/run.rs:156-169`) -> `RunBusy` ("Another application invocation owns this installation", `selection/failure.rs:39`).
- Coordinator: `retain_running_generation` -> `InstallationRun::acquire` (`setup_storage/running.rs:136`), submitted at `ss/production/coordinator_authority/installation.rs:89-92`; failure -> `AuthorityIssue::Stopped` (`:119`) -> `fail`/`request_close` (`coordinator_authority.rs:255-268`). App closes during startup; generic `show_startup_failure` (`desktop/src/startup_error_dialog.rs`). Election already succeeded, so no "already running" surface.
- Launcher, accepted selection: never touches the run lock. Trial recorded: `reserve_installed_launch` (`selection/reservation.rs:117-119`) reads `RunBusy` as live trial, forwards trial generation (`reservation/forwarding.rs:12-45`); coordinator then hits `RunBusy` again or fails `select_launch`; `recover_abandoned_trial` (`run/recovery.rs:50-59`) never runs while the foreign handle stays open.
- Same failure: `transaction/activation.rs:65`, `signed_installer.rs:288`, `signed_installer/installed.rs:116`.
- Other share-0 rendezvous (hard storage errors): `cache/root.rs:65,97`, `user_data/originals.rs:72`, `user_data/invocation/adoption.rs:67`, `durable_storage/imports.rs:88`, `session_identity.rs:67`, `diagnostic_storage/operational/contention.rs:23`, `trust/updater/custody.rs:35`.
- Byte-range writers' rendezvous (`protection.rs:181-183`) shares read+write but requests write access: a foreign `FILE_SHARE_READ`-only handle fails it with `Storage`, not `WriterBusy` (`selection.rs:268-279`).
- Transfer today: `custody::Transfer::new` (`run/transfer/custody.rs:15-29`) duplicates run handle inheritable (`:26`), sent with snapshot via HANDLE_LIST (`transfer.rs:46-58`); trial app captures (`transfer.rs:76-85`), checks in `custody::receive` (`custody.rs:39-68`) and `receive_trial` (`transfer.rs:177-196`). Custody helper gets duplicates from `prepare_witness` (`witness.rs:28-33`) via `registry/custody_establishment.rs:138-167`, received in `native_claims/backend_custody/installation/worker.rs:142-166` -> `TrialWitness::receive_transferred` (`witness.rs:55-100`). Exclusion survives because share mode belongs to the file object (`protection.rs:185-187`).
- `verify_exclusive_rendezvous` (`durable_storage/protection/exclusion.rs:11-43`): security descriptor; regular empty non-reparse file; same identity as canonical path opened with access 0; fresh opens for READ_DATA, WRITE_DATA, DELETE each fail with sharing violation.
- Election/profile locks (`durable_storage/protected/exclusion.rs:116-165`): share read+write open, validated empty regular file + descriptor, std `try_lock` (whole-range exclusive `LockFileEx`), `WouldBlock` = busy. Never transferred; custody helper re-acquires profile writer after parent releases with `WouldBlock` retry (`backend_custody/profile_registration.rs:79-111`) — a gap unacceptable for the run lock.

**Recommended fix: byte-range slots with a gate, acquire-before-release handoff.**
- Rendezvous opened share-all. Gate byte G, slots S0..Sn (n ~ 8) beyond EOF.
- Fresh acquire: lock G fail-fast; lock+unlock whole S0..Sn (conflict = `RunBusy`); lock S0; unlock G.
- Receive: identity check as today; prove sender live (probe lock on S0..Sn fails); lock free slot k >= 1 on inherited handle; then acknowledge (sender holds custody until ack, `witness.rs:16`, `transfer.rs:21-25`).
- New verify: identity, protection, emptiness, plus "this process holds slot k and a fresh whole-range probe fails".
- Offset `LockFileEx` needs a narrow documented FFI adapter in `durable_storage/protection`.
- Same change for `updater-preparation-v1`; move other share-0 users to `rendezvous()` + `try_lock()`; classify `ERROR_LOCK_VIOLATION`/`WouldBlock` as busy.
- Delete `run_exclusion_interoperates_with_existing_byte_range_lock_owners` (`run/tests.rs:227`); product unreleased.

Tests: `run_lock_tolerates_foreign_shared_reader` in `sup/setup_storage/selection/run/tests.rs` (hold `File::open`; `acquire` is `Err(RunBusy)` today, must succeed after fix, second `acquire` still fails); equivalents in `protection/exclusion.rs` tests. `witness_retains_native_exclusion_after_all_sender_custody_is_dropped` (`run/transfer/witness_tests.rs:20`) should fail under a naive swap (same PID). Native cross-process qualification: parent exits, child still holds run.

## C6 / F8 — CONFIRMED across all of `crates/`
- Only deletion paths: `durable_storage/retirement.rs:27-79`, `protection/stage.rs:30-95`, `receiving/lifecycle.rs`, `record_file/streaming/lifecycle.rs`, `native_rename.rs`. Callers: cache (`cache/consumers.rs`, `cache/tree.rs:246-289`, `cache/generated_files/records.rs:182`), diagnostics, stable entry (`entrypoint/stable/transition.rs:71,99`, `payload.rs:95`), `setup_storage/publication.rs:120,211`. None touches `releases\<id>`. Code says so: `contracts/installation/transition.rs:23`, `run/recovery.rs:3,10`, `reservation.rs:104`. Every superseded candidate gets a fresh GUID (`transaction/reservation.rs:53`).
- A generation may go only when: not `accepted`/`retained`/`candidate` (`contracts/installation/record.rs:63-71`); not the stable entry's recovery source (`entrypoint/stable/payload.rs:44`); not referenced by a D22 receipt; remover holds run custody; remover holds updater preparation (reservation compare-exchanges a new id before creating the directory, `transaction/reservation.rs:69-85`); no open handle pins it (launcher pins `reservation.rs:29-33`, mapped images) — the tombstone rename detects these.
- Helpers: `retirement.rs:39-79` (share-0 DELETE open, `OPEN_REPARSE_POINT`, rejects 0x400, identity check, `single_link`); `cache/tree.rs:34-289` declared-ownership walker (unknown content preserved); `files::lexical`, `files::canonical_path` (`entrypoint.rs:51-60`); `ownership::read`, `running_present` (`ownership.rs:50,59`); `Protection::directory` with `Mode::Existing`; cache's `record.deleting` flag.

**Recommended design:** generation-retirement owner in `setup_storage`, a storage job via the execution owner. Runs after the coordinator retains its generation and after a trial settles; needs run custody plus fail-fast preparation (skip turn if busy). Keep: accepted, retained, at most one candidate, stable entry source. For each other GenerationId directory: rename to `releases\.retired-<id>` (atomic, never replaces, fails if anything inside is open; tombstone is the journal); delete only inventoried files, markers and own stage files with `retirement::file`; remove empty dirs leaf-first with `retirement::directory` (lift the `cache/tree.rs` walker into `durable_storage`); unknown files leave the tombstone with one warning. Resume `.retired-*` at startup. Leave trust release history (`trust/installed/publication.rs:54`). No new persisted record.

Tests: superseded candidate, rejected trial, older-previous fixtures; crash between rename and delete resumes; pinned generation survives; unknown file preserved; staging race; junction not followed; stable entry source kept.

## C7 / S10 — PARTIAL
- Partials: `<install>\runtime-cache-v1\update-downloads-v1\<sha key>\...\payload`, keyed by installation, digest, length (`download/custody.rs:41-50`); charged full length; budget 24 GiB namespace / 64 GiB total; 30-day retention (`cache/policy.rs:60-96`).
- Prefix flushed on every outcome (`durable_storage/payload.rs:161-176`); resume from actual length (`download.rs:51`, `update_http.rs:83`); oversized reset (`download.rs:42-45`), digest mismatch reset (`:112-119`), 200 restarts from zero (`payload.rs:198-200`). Mid-body disconnect is `Err`, ends campaign (`update_repository.rs:155-167`; F1). Stale partials reclaimed only in cache admission (`cache/admission.rs:93,109`); no update path calls `maintain()`.
- No `GetDiskFreeSpaceExW` in `crates/`. `StorageFull` from `write_all` (`payload.rs:153`)/sync -> `DownloadFailure::Storage` (`download/failure.rs:60-70`) -> `update_http::Failure::Download` -> `update_repository::Failure::Download`: untyped, ends campaign silently, repeats every run. Same `StorageFull` kind used for logical budget (`cache/maintenance.rs:321-324`, `cache/entry.rs:94,161,191`), so a physically full disk never triggers reclaim. Only setup provisioning handles disk-full (`ss/setup/worker/downloads/service.rs:110`, `provisioning_cache.rs:185`).
- Peak disk ~ archive + extracted `PackedPayloads` entry (`packed.rs:231-253`) + `releases\<id>` copy (`bundle/payload.rs:162-219`).
- Defect: `.payload-stage-<GUID>` (`bundle/payload.rs:198`) never deleted on failure; tests pin the leak (`update_preparation/tests/restart.rs:84`, `packed/tests/downloaded/authorization/recovery.rs:146`).

**Fixes:** typed `InsufficientSpace { needed, available }` with free-space preflight before download and staging; on shortage run `maintain()` and C6 retirement first, then report; map physical `StorageFull` to it and give logical budget its own variant; surface via D8/D4w; C1 backoff treats it as non-transient; stage files via `Protection::transient_file` (`protection.rs:169-177`) + `retain_transient` (`protection/stage.rs:10-26`) before `publish`, updating the two tests; release archive and extracted entry after acceptance.

Tests: `StorageFull` writer mid-chunk (prefix kept, resume correct, error typed); failed stage copy leaves no orphan; preflight shortage reclaims before failing.

## Other defects
- Goal 2: every share-0 rendezvous above can be blocked by an indexer or sync client.
- Goal 1: trial record + foreign handle on run lock -> forwarded trial never admitted, recovery cannot run.
- Goal 5: `run.rs:75` takes the selection writer fail-fast inside run acquisition (A1).
