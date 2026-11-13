// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The host's splash-time update: acquisition as the shared decisions define it, tracked-file
//! replacement with a backup, the three-verdict judgement, and failures that never block.

use super::decisions::{CONFIG, FakeSources, config, install_registry_pack, pack_files};
use super::fixture::folder_files;
use super::*;
use std::path::{Path, PathBuf};

const REGISTRY: &str = "https://api.comfy.org";
const GITHUB: &str = "https://api.github.com";

/// An installed SugarCubes 1.0.0 with user data, the host's storage beside it, and the release
/// sources, which publish nothing yet.
struct Host {
    root: tempfile::TempDir,
    sources: FakeSources,
    config: ReleaseSourceConfig,
}

impl Host {
    fn new() -> std::result::Result<Self, Failure> {
        let root = tempfile::tempdir()?;
        let pack = install_registry_pack(root.path(), &pack_files("1.0.0"))?;
        let described = pack.join(RELEASE_SOURCE_FILE);
        std::fs::create_dir_all(described.parent().ok_or("no parent")?)?;
        std::fs::write(described, CONFIG)?;
        std::fs::write(pack.join("user-notes.txt"), "kept")?;
        let host = Self { root, sources: FakeSources::default(), config: config()? };
        host.sources.publish_registry(
            &host.config,
            "1.0.0",
            "NodeVersionStatusActive",
            pack_files("1.0.0"),
        )?;
        host.sources.empty(host.config.github_releases_url());
        Ok(host)
    }

    fn pack(&self) -> PathBuf {
        self.root.path().join("custom_nodes").join(SUGARCUBES_PACK_ID)
    }

    fn update(&self) -> std::result::Result<StartupUpdate, Failure> {
        Ok(StartupUpdate::at(&self.pack(), &self.root.path().join("host-storage"))?)
    }

    fn run(&self) -> std::result::Result<StartupOutcome, Failure> {
        Ok(self.update()?.run(&self.sources, &|| true))
    }

    fn version(&self) -> std::result::Result<String, Failure> {
        let pyproject = std::fs::read(self.pack().join("pyproject.toml"))?;
        Ok(PackIdentity::parse(&pyproject)?.version.to_string())
    }

    fn publish(&self, version: &str) -> TestResult {
        self.sources.publish_registry(
            &self.config,
            version,
            "NodeVersionStatusFlagged",
            pack_files(version),
        )
    }
}

fn version(text: &str) -> std::result::Result<ReleaseVersion, Failure> {
    Ok(ReleaseVersion::parse(text)?)
}

fn tracked_files(pack: &Path) -> std::result::Result<BTreeMap<String, String>, Failure> {
    let mut files = folder_files(pack)?;
    files.retain(|path, _| {
        !path.starts_with(".sugarcubes/") && !path.starts_with("sugarcubes/backend/")
    });
    Ok(files)
}

#[test]
fn the_newest_version_is_installed_and_kept_once_its_page_loads() -> TestResult {
    let host = Host::new()?;
    host.publish("1.1.0")?;
    let outcome = host.run()?;
    assert_eq!(
        outcome,
        StartupOutcome::Updated {
            from: version("1.0.0")?,
            to: version("1.1.0")?,
            source: ReleaseSourceKind::Registry,
            unchecked: Vec::new(),
        }
    );
    assert_eq!(host.version()?, "1.1.0");
    assert_eq!(std::fs::read_to_string(host.pack().join("user-notes.txt"))?, "kept");
    assert!(!host.root.path().join("host-storage/download").exists());
    let update = host.update()?;
    update.observe(TrialEvent::BackendStartUnobservable)?;
    update.observe(TrialEvent::PageLoaded { nodes_registered: true })?;
    assert!(!host.root.path().join("host-storage/transaction").exists());
    assert_eq!(host.run()?, StartupOutcome::UpToDate);
    Ok(())
}

#[test]
fn a_backend_that_stops_before_ready_restores_the_previous_version_for_good() -> TestResult {
    let host = Host::new()?;
    let before = tracked_files(&host.pack())?;
    host.publish("1.1.0")?;
    assert!(matches!(host.run()?, StartupOutcome::Updated { .. }));
    let restored = host.update()?.backend_stopped()?.ok_or("nothing restored")?;
    assert_eq!(restored.reason, RestoreReason::DidNotStart);
    assert_eq!(host.version()?, "1.0.0");
    assert_eq!(tracked_files(&host.pack())?, before);
    // The version that failed is not installed again; a newer one is.
    assert_eq!(host.run()?, StartupOutcome::UpToDate);
    assert_eq!(host.version()?, "1.0.0");
    host.publish("1.2.0")?;
    assert!(matches!(host.run()?, StartupOutcome::Updated { .. }));
    assert_eq!(host.version()?, "1.2.0");
    Ok(())
}

#[test]
fn a_page_failure_restores_at_the_next_start() -> TestResult {
    let host = Host::new()?;
    host.publish("1.1.0")?;
    host.run()?;
    let update = host.update()?;
    update.observe(TrialEvent::BackendStartUnobservable)?;
    update.observe(TrialEvent::PageFailed)?;
    match host.run()? {
        StartupOutcome::Restored(restored) => {
            assert_eq!(restored.reason, RestoreReason::PageDidNotLoad);
            assert_eq!(restored.restored_version, "1.0.0");
        }
        other => return Err(format!("not restored: {other:?}").into()),
    }
    assert_eq!(host.version()?, "1.0.0");
    Ok(())
}

#[test]
fn starts_the_host_never_witnessed_keep_the_update_until_they_count_as_one_failure() -> TestResult {
    let host = Host::new()?;
    host.publish("1.1.0")?;
    host.run()?;
    for _ in 1..UNWITNESSED_ENDINGS_LIMIT {
        assert_eq!(host.run()?, StartupOutcome::Judging { version: "1.1.0".into() });
        assert_eq!(host.version()?, "1.1.0");
    }
    assert!(matches!(host.run()?, StartupOutcome::Restored(_)));
    assert_eq!(host.version()?, "1.0.0");
    Ok(())
}

#[test]
fn unanswered_sources_and_failed_downloads_leave_the_installed_version() -> TestResult {
    let host = Host::new()?;
    let before = tracked_files(&host.pack())?;
    host.sources.down.borrow_mut().extend([REGISTRY.to_owned(), GITHUB.to_owned()]);
    assert_eq!(
        host.run()?,
        StartupOutcome::CouldNotCheck {
            unchecked: vec![ReleaseSourceKind::Registry, ReleaseSourceKind::GitHubRelease]
        }
    );
    host.sources.down.borrow_mut().clear();
    host.sources.publish_github(&host.config, "1.1.0", false, false)?;
    host.sources.corrupt.borrow_mut().insert(
        "https://github.com/Artificial-Sweetener/SugarCubes/releases/download/sugarcubes-v1.1.0/SugarCubes-1.1.0.zip"
            .into(),
    );
    assert_eq!(
        host.run()?,
        StartupOutcome::Failed { step: StartupStep::Downloading, code: "digest-mismatch" }
    );
    host.sources.down.borrow_mut().insert("https://github.com".into());
    assert_eq!(
        host.run()?,
        StartupOutcome::Failed { step: StartupStep::Downloading, code: "source" }
    );
    assert_eq!(tracked_files(&host.pack())?, before);
    assert!(!host.root.path().join("host-storage/transaction").exists());
    Ok(())
}

#[test]
fn folders_the_host_may_not_replace_are_left_exactly_as_they_are() -> TestResult {
    let host = Host::new()?;
    host.publish("1.1.0")?;
    std::fs::create_dir(host.pack().join(".git"))?;
    let before = folder_files(&host.pack())?;
    assert_eq!(host.run()?, StartupOutcome::NotManaged(NotOwnedReason::GitCheckout));
    assert_eq!(folder_files(&host.pack())?, before);
    std::fs::remove_dir(host.pack().join(".git"))?;
    std::fs::write(host.pack().join("sugarcubes/core.py"), "# my edit\n")?;
    assert_eq!(host.run()?, StartupOutcome::NotManaged(NotOwnedReason::ChangedFiles));
    assert_eq!(std::fs::read_to_string(host.pack().join("sugarcubes/core.py"))?, "# my edit\n");
    Ok(())
}

#[test]
fn sugarcubes_own_updater_holding_the_folder_makes_the_host_wait_for_the_next_start() -> TestResult
{
    let host = Host::new()?;
    host.publish("1.1.0")?;
    let lock = host.pack().join(UPDATE_LOCK_FILE);
    std::fs::create_dir_all(lock.parent().ok_or("no parent")?)?;
    let held =
        std::fs::File::options().read(true).write(true).create(true).truncate(false).open(&lock)?;
    held.try_lock()?;
    assert_eq!(host.run()?, StartupOutcome::Busy);
    assert_eq!(host.version()?, "1.0.0");
    // Closing the handle releases the lock, as SugarCubes' updater exiting does.
    drop(held);
    assert!(matches!(host.run()?, StartupOutcome::Updated { .. }));
    Ok(())
}

#[test]
fn a_cancelled_or_expired_worker_stops_before_changing_anything() -> TestResult {
    let host = Host::new()?;
    host.publish("1.1.0")?;
    assert_eq!(host.update()?.run(&host.sources, &|| false), StartupOutcome::Stopped);
    assert_eq!(host.version()?, "1.0.0");
    Ok(())
}

#[test]
fn a_missing_release_description_fails_the_check_without_blocking() -> TestResult {
    let host = Host::new()?;
    std::fs::remove_file(host.pack().join(RELEASE_SOURCE_FILE))?;
    assert_eq!(
        host.run()?,
        StartupOutcome::Failed { step: StartupStep::Checking, code: "release-source" }
    );
    Ok(())
}

/// The whole check, download and install over the host's real HTTP owner and WinHTTP, against
/// loopback stand-ins for the Registry API and its archive host.
#[test]
fn a_real_http_check_installs_the_registry_release_over_loopback() -> TestResult {
    use super::fixture::{file_members, zip_bytes};
    use crate::setup_storage::test_worker as worker;
    use crate::update_http::{
        Http,
        fixture::{Running, complete},
    };
    worker::run_reserved(STARTUP_UPDATE_RESERVATION_BYTES, |context, _| {
        let host = Host::new()?;
        let archive = zip_bytes(&file_members(&pack_files("1.1.0")))?;
        let (archives, archive_policy) =
            Running::start(vec![complete("cdn/SugarCubes/1.1.0/node.zip", &archive)], context)?;
        let archive_port = archive_policy.asset("x")?.port()?;
        let download = format!("http://127.0.0.1:{archive_port}/cdn/SugarCubes/1.1.0/node.zip");
        let versions = serde_json::json!([
            {"node_id": "SugarCubes", "version": "1.0.0", "status": "NodeVersionStatusActive", "downloadUrl": download.replace("1.1.0", "1.0.0")},
            {"node_id": "SugarCubes", "version": "1.1.0", "status": "NodeVersionStatusFlagged", "downloadUrl": download},
        ])
        .to_string();
        let (api, api_policy) = Running::start(
            vec![
                complete("nodes/SugarCubes/versions", versions.as_bytes()),
                complete("repos/Artificial-Sweetener/SugarCubes/releases?per_page=50", b"[]"),
            ],
            context,
        )?;
        let api_root = format!("http://127.0.0.1:{}", api_policy.asset("x")?.port()?);
        let described = CONFIG
            .replace("https://api.comfy.org", &api_root)
            .replace("https://api.github.com", &api_root)
            .replace("cdn.comfy.org", "127.0.0.1")
            .replace(
                r#"["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"]"#,
                r#"["127.0.0.1"]"#,
            );
        std::fs::write(host.pack().join(RELEASE_SOURCE_FILE), described)?;
        let mut http = Http::new();
        let outcome = {
            let sources = HttpReleaseSources::with_policies(
                &mut http,
                [api_policy, archive_policy],
                &host.root.path().join("host-storage/published"),
                context,
            )?;
            host.update()?.run(&sources, &|| !context.cancellation_requested())
        };
        assert!(http.settled());
        let client = (|| -> TestResult {
            assert_eq!(
                outcome,
                StartupOutcome::Updated {
                    from: version("1.0.0")?,
                    to: version("1.1.0")?,
                    source: ReleaseSourceKind::Registry,
                    unchecked: Vec::new(),
                }
            );
            assert_eq!(host.version()?, "1.1.0");
            Ok(())
        })();
        let client = api.finish_after(client);
        archives.finish_after(client)
    })
}

#[test]
fn every_run_leaves_its_update_report_and_shows_one_plain_line_when_needed() -> TestResult {
    use sugarsubstitute_contracts::localization::pack_updates::EN;
    let host = Host::new()?;
    host.publish("1.1.0")?;
    let outcome = host.run()?;
    assert_eq!(
        outcome.report().notice(&EN).as_deref(),
        Some("SugarCubes updated to version 1.1.0")
    );
    let report: Value = serde_json::from_slice(&std::fs::read(
        host.root.path().join("host-storage").join(REPORT_RECORD),
    )?)?;
    assert_eq!(
        report,
        serde_json::json!({"schema": 1, "outcome": "updated", "from": "1.0.0", "to": "1.1.0", "source": "registry"})
    );
    host.sources.down.borrow_mut().extend([REGISTRY.to_owned(), GITHUB.to_owned()]);
    let failed = StartupOutcome::CouldNotCheck {
        unchecked: vec![ReleaseSourceKind::Registry, ReleaseSourceKind::GitHubRelease],
    };
    assert_eq!(failed.report().notice(&EN).as_deref(), Some(EN.could_not_check));
    assert_eq!(
        failed.report().lines(&EN),
        [EN.could_not_check, EN.registry_unchecked, EN.github_unchecked]
    );
    assert_eq!(StartupOutcome::UpToDate.report().notice(&EN), None);
    assert_eq!(
        StartupOutcome::NotManaged(NotOwnedReason::GitCheckout).report().lines(&EN),
        [EN.kept_git]
    );
    Ok(())
}

#[test]
fn a_comfyui_the_host_does_not_start_is_only_reported_on() -> TestResult {
    use sugarsubstitute_contracts::localization::pack_updates::EN;
    let host = Host::new()?;
    host.publish("1.1.0")?;
    let before = folder_files(&host.pack())?;
    let outcome = host.update()?.check(&host.sources);
    assert_eq!(
        outcome,
        StartupOutcome::Available { version: version("1.1.0")?, unchecked: Vec::new() }
    );
    assert_eq!(
        outcome.report().notice(&EN).as_deref(),
        Some("SugarCubes version 1.1.0 is available.")
    );
    assert_eq!(folder_files(&host.pack())?, before);
    assert!(!host.root.path().join("host-storage/transaction").exists());
    assert!(!host.pack().join(".sugarcubes").exists());
    Ok(())
}

#[test]
fn a_failed_page_restores_the_previous_version_before_the_next_attempt() -> TestResult {
    use sugarsubstitute_contracts::update::pack::{PackOutcome, PackUpdateReport};
    let host = Host::new()?;
    host.publish("1.1.0")?;
    host.run()?;
    let update = host.update()?;
    // Nothing to restore while the new version has not failed its page.
    assert_eq!(update.restore_failed_page()?, None);
    update.observe(TrialEvent::BackendStartUnobservable)?;
    assert_eq!(update.restore_failed_page()?, None);
    assert_eq!(host.version()?, "1.1.0");
    update.observe(TrialEvent::PageFailed)?;
    let restored = update.restore_failed_page()?.ok_or("nothing restored")?;
    assert_eq!(restored.reason, RestoreReason::PageDidNotLoad);
    assert_eq!(host.version()?, "1.0.0");
    let report = PackUpdateReport::parse(&std::fs::read(
        host.root.path().join("host-storage").join(REPORT_RECORD),
    )?)?;
    assert_eq!(report.outcome, PackOutcome::Restored);
    assert_eq!(report.reason.as_deref(), Some("page-did-not-load"));
    // The failed version is not tried again; the next start is up to date.
    assert_eq!(host.run()?, StartupOutcome::UpToDate);
    Ok(())
}
