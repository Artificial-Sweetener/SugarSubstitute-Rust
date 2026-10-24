// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Embedded distribution parsing reaches the real signed installer verifier and native journal.
use super::*;
use crate::{
    durable_storage::files::Directory,
    setup_storage::{
        existing::fixture,
        test_package::{self, repository::SignedPackage},
        test_worker as worker,
        test_worker::TestResult,
    },
};
use sha2::{Digest, Sha256};
use sugarsubstitute_update::fixture::{EXPIRES, NOW};
type Failure = Box<dyn std::error::Error + Send + Sync>;
mod reuse;

#[test]
fn retained_preview_and_signed_import_complete_on_the_same_execution_ledger() -> TestResult {
    use crate::setup_storage::test_worker::pipeline::Pipeline;
    let mut pipeline = Pipeline::new()?;
    let (fixture, preview) =
        pipeline.run(crate::setup_storage::MATERIALIZATION_RESERVATION_BYTES, |context| {
            let fixture = Fixture::new(&context)?;
            let distribution = InstallerDistribution::decode(
                &serde_json::to_vec(&fixture.configuration)?,
                &context,
            )?;
            let digest = distribution.manifest_digest().to_owned();
            drop(distribution);
            let root = crate::setup_storage::materialize(
                &fixture.executable,
                &crate::setup_storage::cache::RuntimeCache::open(
                    fixture.workspace.path(),
                    crate::setup_storage::cache::CachePolicy::default(),
                    &context,
                )?,
                &digest,
                &context,
            )?;
            let preview = Arc::new(root);
            Ok((fixture, preview))
        })?;
    let retained = Arc::clone(&preview);
    let (fixture, imported) = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let distribution =
            InstallerDistribution::decode(&serde_json::to_vec(&fixture.configuration)?, &context)?;
        let imported = distribution.import(
            InstallationSession::maintain(fixture.root.path(), &context)?,
            &fixture.executable,
            BundleSource::Retained(retained),
            || NOW,
            &context,
        )?;
        assert_eq!(
            std::fs::read_dir(
                fixture
                    .workspace
                    .path()
                    .join(crate::setup_storage::cache::DIRECTORY)
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
            )?
            .count(),
            1,
            "Signed import must reuse the retained verified preview"
        );
        Ok((fixture, imported))
    })?;
    let prepared = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        Ok(imported.prepare(|| NOW, &context)?)
    })?;
    let candidate = pipeline.run(SIGNED_PREPARATION_RESERVATION_BYTES, move |context| {
        prepared.stage(&context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime absent")?.clone();
        Ok(prepared.complete(&runtime, || NOW, &context)?)
    })?;
    assert!(preview.worker_paths().0.is_file());
    assert!(candidate.directory().join("sugarsubstitute.exe").is_file());
    assert_eq!(
        std::fs::read(candidate.directory().join("startup.local.toml"))?,
        std::fs::read(fixture.root.path().join("releases/installed/startup.local.toml"))?
    );
    drop(candidate);
    drop(preview);
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |_| Ok(()))?;
    Ok(())
}

struct Fixture {
    root: tempfile::TempDir,
    workspace: tempfile::TempDir,
    executable: PathBuf,
    configuration: serde_json::Value,
}
impl Fixture {
    fn new(context: &JobContext) -> Result<Self, Failure> {
        let root = tempfile::tempdir()?;
        fixture::installed(&Directory::open(root.path(), false)?, context)?;
        let workspace = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution()?;
        let archive = test_package::seal(&manifest, &compressed);
        let signed = SignedPackage::new(&archive, 1)?;
        let executable = workspace.path().join("setup.exe");
        std::fs::write(&executable, [b"inert installer prefix".as_slice(), &archive].concat())?;
        let configuration = serde_json::json!({
            "schema_version":1, "repository":vec![92u8;32], "channel":"release",
            "manifest_sha256":format!("{:x}", Sha256::digest(&manifest)),
            "trusted_root":String::from_utf8(signed.root)?, "roots":[],
            "timestamp":String::from_utf8(signed.metadata.timestamp)?,
            "snapshot":String::from_utf8(signed.metadata.snapshot)?,
            "targets":String::from_utf8(signed.metadata.targets)?,
            "descriptor":String::from_utf8(signed.descriptor)?,
        });
        Ok(Self { root, workspace, executable, configuration })
    }
    fn import(&self, now: i64, context: &JobContext) -> Result<SignedInstallation, Failure> {
        let distribution =
            InstallerDistribution::decode(&serde_json::to_vec(&self.configuration)?, context)?;
        Ok(distribution.import(
            InstallationSession::maintain(self.root.path(), context)?,
            &self.executable,
            BundleSource::Extract(self.workspace.path()),
            || now,
            context,
        )?)
    }
}

#[test]
fn embedded_distribution_completes_signed_candidate_with_preserved_configuration() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        let prepared = fixture.import(NOW, context)?.prepare(|| NOW, context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Saved runtime missing")?.clone();
        let candidate = prepared.complete(&runtime, || NOW, context)?;
        assert_eq!(
            std::fs::read(candidate.directory().join("startup.local.toml"))?,
            std::fs::read(fixture.root.path().join("releases/installed/startup.local.toml"))?
        );
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("No state")?
                .accepted()
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn wrong_inventory_channel_and_expired_roles_never_reserve_a_candidate() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        for case in 0..3 {
            let mut fixture = Fixture::new(context)?;
            if case == 0 {
                fixture.configuration["manifest_sha256"] = serde_json::json!("f".repeat(64));
            }
            if case == 1 {
                fixture.configuration["channel"] = serde_json::json!("canary");
            }
            assert!(fixture.import(if case == 2 { EXPIRES } else { NOW }, context).is_err());
            let state = SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("No state")?;
            assert!(state.candidate().is_none());
            assert_eq!(std::fs::read_dir(fixture.root.path().join("releases"))?.count(), 1);
        }
        Ok(())
    })
}

#[test]
fn distribution_rejects_unknown_schema_fields_bounds_and_root_order() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let fixture = Fixture::new(context)?;
        for case in 0..7 {
            let mut value = fixture.configuration.clone();
            match case {
                0 => value["schema_version"] = serde_json::json!(2),
                1 => value["foreign"] = serde_json::json!(true),
                2 => value["repository"] = serde_json::json!(vec![0u8; 32]),
                3 => value["manifest_sha256"] = serde_json::json!("F".repeat(64)),
                4 => value["timestamp"] = serde_json::json!("x".repeat(MAX_METADATA_BYTES + 1)),
                5 => {
                    value["roots"] = serde_json::json!([{"version":2,"metadata":"{}"},{"version":2,"metadata":"{}"}])
                }
                _ => value["trusted_root"] = serde_json::json!(""),
            }
            assert!(InstallerDistribution::decode(&serde_json::to_vec(&value)?, context).is_err());
        }
        assert!(InstallerDistribution::decode(&[], context).is_err());
        assert!(
            InstallerDistribution::decode(&vec![b' '; MAX_DISTRIBUTION_BYTES + 1], context)
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn decoded_roles_retain_exact_signed_bytes_and_do_not_grant_signature_authority() -> TestResult {
    worker::run_reserved(SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let mut fixture = Fixture::new(context)?;
        let timestamp =
            format!("\n{}\n", fixture.configuration["timestamp"].as_str().ok_or("No timestamp")?);
        fixture.configuration["timestamp"] = serde_json::json!(timestamp);
        let distribution =
            InstallerDistribution::decode(&serde_json::to_vec(&fixture.configuration)?, context)?;
        assert_eq!(distribution.configuration.timestamp, timestamp);
        drop(distribution);
        fixture.configuration["targets"] = serde_json::json!("{}");
        assert!(fixture.import(NOW, context).is_err());
        assert!(
            SelectionStore::open(fixture.root.path(), context)?
                .load(context)?
                .ok_or("No state")?
                .candidate()
                .is_none()
        );
        Ok(())
    })
}
