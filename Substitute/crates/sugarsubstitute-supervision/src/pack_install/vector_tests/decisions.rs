// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the source-selection, refusal and rollback vectors through the host's decisions.

use super::fixture::{apply_edits, build_folder, exists, file_members, read, zip_bytes};
use super::*;
use crate::setup_storage::test_worker;
use serde_json::json;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

const CONFIG: &str = r#"{"schema": 1, "packId": "SugarCubes",
  "registry": {"apiRoot": "https://api.comfy.org", "archiveHost": "cdn.comfy.org"},
  "githubRelease": {"apiRoot": "https://api.github.com", "repository": "Artificial-Sweetener/SugarCubes",
    "tagPrefix": "sugarcubes-v", "assetName": "SugarCubes-{version}.zip",
    "downloadHosts": ["github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"]}}"#;

fn config() -> std::result::Result<ReleaseSourceConfig, Failure> {
    Ok(ReleaseSourceConfig::parse(CONFIG.as_bytes())?)
}

/// The files of a small SugarCubes release, as SugarCubes' own fixture builds them.
pub(super) fn pack_files(version: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "pyproject.toml".into(),
            format!("[project]\nname = \"SugarCubes\"\nversion = \"{version}\"\n"),
        ),
        ("__init__.py".into(), format!("VERSION = '{version}'\n")),
        ("sugarcubes/core.py".into(), format!("# core {version}\n")),
        ("web/app.js".into(), format!("// app {version}\n")),
    ])
}

/// The Registry and GitHub endpoints, faked at the network boundary.
#[derive(Default)]
struct FakeSources {
    replies: RefCell<BTreeMap<String, Value>>,
    archives: RefCell<BTreeMap<String, BTreeMap<String, String>>>,
    down: RefCell<BTreeSet<String>>,
}

impl ReleaseSources for FakeSources {
    fn fetch_json(&self, url: &str) -> std::result::Result<Value, SourceFailure> {
        if self.down.borrow().iter().any(|prefix| url.starts_with(prefix)) {
            return Err(SourceFailure::Unavailable);
        }
        self.replies.borrow().get(url).cloned().ok_or(SourceFailure::NotFound)
    }

    fn published_digests(
        &self,
        offer: &ReleaseOffer,
    ) -> std::result::Result<FileDigests, SourceFailure> {
        let archives = self.archives.borrow();
        let files = archives.get(&offer.archive_url).ok_or(SourceFailure::NotFound)?;
        Ok(files.iter().map(|(path, text)| (path.clone(), sha256(text.as_bytes()))).collect())
    }
}

impl FakeSources {
    fn publish_registry(
        &self,
        config: &ReleaseSourceConfig,
        version: &str,
        status: &str,
        files: BTreeMap<String, String>,
    ) -> TestResult {
        let url =
            format!("https://cdn.comfy.org/artificialsweetener/SugarCubes/{version}/node.zip");
        let record = json!({"node_id": "SugarCubes", "version": version, "status": status, "downloadUrl": url});
        let mut replies = self.replies.borrow_mut();
        replies
            .insert(config.registry_install_url(ReleaseVersion::parse(version)?), record.clone());
        let listing = replies.entry(config.registry_versions_url()).or_insert_with(|| json!([]));
        listing.as_array_mut().ok_or("listing")?.push(record);
        self.archives.borrow_mut().insert(url, files);
        Ok(())
    }

    fn publish_github(
        &self,
        config: &ReleaseSourceConfig,
        version: &str,
        draft: bool,
        prerelease: bool,
    ) -> TestResult {
        let files = pack_files(version);
        let archive = zip_bytes(&file_members(&files))?;
        let tag = format!("sugarcubes-v{version}");
        let asset = format!("SugarCubes-{version}.zip");
        let url = format!(
            "https://github.com/Artificial-Sweetener/SugarCubes/releases/download/{tag}/{asset}"
        );
        let release = json!({"tag_name": tag, "draft": draft, "prerelease": prerelease, "assets": [{
            "name": asset, "state": "uploaded", "size": archive.len(),
            "digest": format!("sha256:{}", sha256(&archive)), "browser_download_url": url}]});
        let mut replies = self.replies.borrow_mut();
        replies.insert(config.github_release_url(ReleaseVersion::parse(version)?), release.clone());
        let listing = replies.entry(config.github_releases_url()).or_insert_with(|| json!([]));
        listing.as_array_mut().ok_or("listing")?.push(release);
        self.archives.borrow_mut().insert(url, files);
        Ok(())
    }

    fn empty(&self, url: String) {
        self.replies.borrow_mut().entry(url).or_insert_with(|| json!([]));
    }
}

/// Install `files` the way this format leaves a registry install: files, `.tracking`, record.
pub(super) fn install_registry_pack(
    root: &Path,
    files: &BTreeMap<String, String>,
) -> std::result::Result<PathBuf, Failure> {
    let pack = root.join("custom_nodes").join("SugarCubes");
    let extracted = archive::extract_registry_archive(
        &zip_bytes(&file_members(files))?,
        &PackTree::at(&pack)?,
        ArchiveLimits::default(),
    )?;
    let version =
        PackIdentity::parse(files.get("pyproject.toml").ok_or("no pyproject")?.as_bytes())?.version;
    ownership::write_ownership_records(
        &PackTree::at(&pack)?,
        &extracted.entries,
        &version.to_string(),
        &extracted.file_digests,
    )?;
    Ok(pack)
}

fn unchecked(kinds: &[ReleaseSourceKind]) -> Vec<String> {
    kinds.iter().map(|kind| kind.as_str().to_owned()).collect()
}

#[test]
fn source_selection_vectors() -> TestResult {
    let config = config()?;
    for vector in drive("sourceSelection")? {
        let name = text(&vector, "name")?;
        let temporary = tempfile::tempdir()?;
        let pack =
            install_registry_pack(temporary.path(), &pack_files(text(&vector, "installed")?))?;
        let sources = FakeSources::default();
        let registry = field(&vector, "registry")?;
        if registry.get("unavailable").is_some() {
            sources.down.borrow_mut().insert(config.registry_versions_url());
        } else {
            sources.empty(config.registry_versions_url());
            for item in list(registry, "versions")? {
                let version = text(item, "version")?;
                sources.publish_registry(
                    &config,
                    version,
                    text(item, "status")?,
                    pack_files(version),
                )?;
            }
        }
        let github = field(&vector, "github")?;
        if github.get("unavailable").is_some() {
            sources.down.borrow_mut().insert(config.github_releases_url());
        } else {
            sources.empty(config.github_releases_url());
            for item in list(github, "releases")? {
                let flag = |key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
                sources.publish_github(
                    &config,
                    text(item, "version")?,
                    flag("draft"),
                    flag("prerelease"),
                )?;
            }
        }
        let decision = PackFolder::at(&pack)?.plan_update(&config, &sources)?;
        let expect = field(&vector, "expect")?;
        let expected_unchecked = texts(expect, "unchecked")?;
        match (text(expect, "outcome")?, decision) {
            ("staged", UpdateDecision::Install { offer, unchecked: kinds, .. }) => {
                assert_eq!(offer.version.to_string(), text(expect, "version")?, "{name}");
                assert_eq!(offer.source.as_str(), text(expect, "source")?, "{name}");
                assert_eq!(unchecked(&kinds), expected_unchecked, "{name}");
            }
            ("up-to-date", UpdateDecision::UpToDate { unchecked: kinds })
            | ("check-failed", UpdateDecision::CheckFailed { unchecked: kinds }) => {
                assert_eq!(unchecked(&kinds), expected_unchecked, "{name}");
            }
            (outcome, decision) => {
                return Err(format!("{name}: expected {outcome}, got {decision:?}").into());
            }
        }
    }
    Ok(())
}

fn with_version(
    files: &Value,
    version: &str,
) -> std::result::Result<BTreeMap<String, String>, Failure> {
    let mut files: BTreeMap<String, String> = serde_json::from_value(files.clone())?;
    files.insert(
        "pyproject.toml".into(),
        format!("[project]\nname = \"SugarCubes\"\nversion = \"{version}\"\n"),
    );
    Ok(files)
}

fn record_digests(pack: &Path, version: &str) -> TestResult {
    let tree = PackTree::at(pack)?;
    let entries = ownership::read_tracking(&tree)?.ok_or("no .tracking")?;
    let files = entries
        .iter()
        .filter(|entry| !entry.is_directory())
        .map(|entry| Ok((entry.path().to_owned(), tree.sha256(entry.path())?)))
        .collect::<std::result::Result<FileDigests, Failure>>()?;
    let tracking = tree.read(TRACKING_FILE_NAME, MAX_TRACKING_BYTES)?;
    ownership::write_digest_record(&tree, version, &sha256(&tracking), &files)?;
    Ok(())
}

#[test]
fn update_refusal_vectors() -> TestResult {
    test_worker::run(|context, _| {
        let config = config()?;
        for vector in drive("updateRefusals")? {
            let name = text(&vector, "name")?;
            let temporary = tempfile::tempdir()?;
            let installed = field(&vector, "installed")?;
            let pack = build_folder(temporary.path(), installed, context)?;
            let recorded = field(&vector, "digestRecord")?.as_bool() == Some(true);
            if recorded {
                record_digests(&pack, "1.0.0")?;
            }
            apply_edits(&pack, field(&vector, "edits")?)?;
            let sources = FakeSources::default();
            sources.empty(config.github_releases_url());
            let files = field(installed, "files")?;
            if !recorded {
                sources.publish_registry(
                    &config,
                    "1.0.0",
                    "NodeVersionStatusActive",
                    serde_json::from_value(files.clone())?,
                )?;
            }
            let newer = text(&vector, "newer")?;
            sources.publish_registry(
                &config,
                newer,
                "NodeVersionStatusActive",
                with_version(files, newer)?,
            )?;
            let decision = PackFolder::at(&pack)?.plan_update(&config, &sources)?;
            let expect = field(&vector, "expect")?;
            match (text(expect, "outcome")?, decision) {
                ("staged", UpdateDecision::Install { offer, .. }) => {
                    assert_eq!(offer.version.to_string(), text(expect, "version")?, "{name}");
                }
                ("not-managed", UpdateDecision::NotManaged(reason)) => {
                    assert_eq!(reason.as_str(), text(expect, "reason")?, "{name}");
                }
                (outcome, decision) => {
                    return Err(format!("{name}: expected {outcome}, got {decision:?}").into());
                }
            }
        }
        Ok(())
    })
}

/// Install 1.0.0, then install 1.1.0 through the host transaction, halting where asked.
fn installed_update(
    root: &Path,
    halt_in: Option<JournalPhase>,
) -> std::result::Result<(PathBuf, replacement::TrackedReplacement), Failure> {
    let pack = install_registry_pack(root, &pack_files("1.0.0"))?;
    let target = PackTree::at(&pack)?;
    let ownership::PackOwnership::Owned(owned) = ownership::inspect_pack_folder(&target)? else {
        return Err("installed pack is not owned".into());
    };
    let staged = PackTree::at(&root.join("staged"))?;
    let extracted = archive::extract_registry_archive(
        &zip_bytes(&file_members(&pack_files("1.1.0")))?,
        &staged,
        ArchiveLimits::default(),
    )?;
    let transaction = replacement::TrackedReplacement::with_faults(
        PackTree::at(&root.join("transaction"))?,
        None,
        halt_in,
    );
    let applied = transaction.apply(&replacement::ReplacementPlan {
        target: &target,
        source: &staged,
        old_entries: &owned.entries,
        new_entries: &extracted.entries,
        from_version: "1.0.0",
        to_version: "1.1.0",
    });
    if halt_in.is_none() {
        applied?;
    } else if applied.is_ok() {
        return Err("the replacement did not halt".into());
    }
    Ok((pack, replacement::TrackedReplacement::new(PackTree::at(&root.join("transaction"))?)))
}

fn installed_version(pack: &Path) -> std::result::Result<String, Failure> {
    Ok(PackIdentity::parse(&read(&pack.join("pyproject.toml"))?)?.version.to_string())
}

#[test]
fn rollback_trigger_vectors() -> TestResult {
    for vector in drive("rollbackTriggers")? {
        let name = text(&vector, "name")?;
        let temporary = tempfile::tempdir()?;
        let mut events = texts(&vector, "events")?;
        let interrupted = events.first().map(String::as_str) == Some("power-cut-while-replacing");
        if interrupted {
            events.remove(0);
        }
        let (pack, transaction) =
            installed_update(temporary.path(), interrupted.then_some(JournalPhase::Replacing))?;
        let target = PackTree::at(&pack)?;
        let mut last = None;
        for event in &events {
            let observation = match event.as_str() {
                "backend-ready" => TrialEvent::BackendReady { nodes_registered: true },
                "backend-ready-nodes-missing" => {
                    TrialEvent::BackendReady { nodes_registered: false }
                }
                "backend-start-unobservable" => TrialEvent::BackendStartUnobservable,
                "page-loaded" => TrialEvent::PageLoaded { nodes_registered: true },
                "page-loaded-nodes-missing" => TrialEvent::PageLoaded { nodes_registered: false },
                "page-failed" => TrialEvent::PageFailed,
                "restart" => {
                    last = transaction.settle(&target, TrialEnding::BackendStopped)?;
                    continue;
                }
                other => return Err(format!("{name}: unknown rollback event {other}").into()),
            };
            transaction.observe("1.1.0", observation)?;
        }
        let expect = field(&vector, "expect")?;
        assert_eq!(installed_version(&pack)?, text(expect, "version")?, "{name}");
        let backup_held = exists(&temporary.path().join("transaction"));
        match text(expect, "verdict")? {
            "restored" => {
                let restored = last.ok_or_else(|| format!("{name}: nothing was restored"))?;
                assert_eq!(restored.reason.as_str(), text(expect, "reason")?, "{name}");
                assert!(!backup_held, "{name}");
                assert_eq!(read(&pack.join("web").join("app.js"))?, b"// app 1.0.0\n", "{name}");
            }
            "kept" => assert!(!backup_held, "{name}"),
            "pending" => assert!(backup_held, "{name}"),
            other => return Err(format!("{name}: unknown verdict {other}").into()),
        }
    }
    Ok(())
}
