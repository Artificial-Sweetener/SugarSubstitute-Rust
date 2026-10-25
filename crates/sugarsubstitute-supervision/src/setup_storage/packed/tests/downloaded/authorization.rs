// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hostile authorization changes exercise the actual downloaded installation publication path.
use super::*;
use local::Delivery;
use std::path::PathBuf;
use sugarsubstitute_contracts::update::Component;

struct Case<'a> {
    directory: &'a Directory,
    original: &'a Path,
    archive: &'a [u8],
    signed: &'a SignedPackage,
    trust: &'a TrustStore,
    prepared: &'a PreparedInstallation,
    runtime: &'a crate::setup_storage::RuntimePaths,
    context: &'a JobContext,
}
impl Case<'_> {
    fn preserved(&self) -> TestResult {
        assert_eq!(
            entrypoint::selected(self.directory, self.context)?.ok_or("Selection lost")?.path(),
            self.original
        );
        assert_eq!(
            std::fs::read(self.original.join("startup.local.toml"))?,
            b"schema_version = 1\r\n# User customization\r\n[backend]\r\nport = 9191\r\n"
        );
        assert!(!self.runtime.python.exists());
        assert!(!self.runtime.directory.exists());
        Ok(())
    }
}

fn run(sequence: u32, body: impl FnOnce(Case<'_>) -> TestResult + Send + 'static) -> TestResult {
    run_installation(sequence, false, body)
}

/// Exercise maintenance after a real candidate was completed and accepted at the storage boundary.
fn run_accepted(body: impl FnOnce(Case<'_>) -> TestResult + Send + 'static) -> TestResult {
    run_installation(2, true, body)
}

fn run_installation(
    sequence: u32,
    accepted: bool,
    body: impl FnOnce(Case<'_>) -> TestResult + Send + 'static,
) -> TestResult {
    run_distribution(sequence, accepted, test_package::distribution()?, body)
}

/// Vary authenticated compatibility metadata while retaining the same actual installation path.
fn run_distribution(
    sequence: u32,
    accepted: bool,
    distribution: (Vec<u8>, Vec<u8>),
    body: impl FnOnce(Case<'_>) -> TestResult + Send + 'static,
) -> TestResult {
    run_source(sequence, accepted, distribution, Delivery::Network, body)
}

/// Vary only the external delivery boundary; authenticated publication remains the real owner.
fn run_source(
    sequence: u32,
    accepted: bool,
    distribution: (Vec<u8>, Vec<u8>),
    delivery: Delivery,
    body: impl FnOnce(Case<'_>) -> TestResult + Send + 'static,
) -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 48 * 1024 * 1024, move |context, clock| {
        clock.checkpoint("fixture_directories");
        let temporary = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let original = if accepted {
            clock.checkpoint("accepted_fixture");
            launch::accept_fixture(&directory, context)?;
            directory.path().join("releases/installed")
        } else {
            clock.checkpoint("unselected_fixture");
            fixture::installed(&directory, context)?
        };
        let session =
            crate::setup_storage::InstallationSession::maintain(directory.path(), context)?;
        let (manifest, compressed) = distribution;
        clock.checkpoint("sign_fixture_metadata");
        let archive = test_package::seal(&manifest, &compressed);
        let signed = SignedPackage::new(&archive, sequence)?;
        clock.checkpoint("open_or_initialize_trust");
        let trust = if accepted {
            TrustStore::open(directory.path(), [92; 32], context)?
        } else {
            signed.initialize(directory.path(), context)?
        };
        clock.checkpoint("authenticated_offer");
        let target = trust
            .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
            .into_package()?;
        clock.checkpoint("package_delivery");
        let download =
            delivery.receive(workspace.path(), &archive, directory.path(), target, context)?;
        clock.checkpoint("extract_and_verify_bundle");
        let bundle = Arc::new(VerifiedBundle::from_download(
            download,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?);
        clock.checkpoint("reserve_maintenance_generation");
        let prepared = session.prepare(bundle, context)?;
        clock.checkpoint("stage_application_payload");
        prepared.stage(context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Missing runtime")?;
        clock.checkpoint("test_behavior_and_retirement");
        body(Case {
            directory: &directory,
            original: &original,
            archive: &archive,
            signed: &signed,
            trust: &trust,
            prepared: &prepared,
            runtime,
            context,
        })
    })
}

mod local;

#[test]
fn withdrawn_cached_package_cannot_publish_and_new_metadata_floors_survive_rejection() -> TestResult
{
    run(1, |case| {
        let newer = SignedPackage::new(b"new signed distribution", 2)?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &newer.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()?;
        let reopened = TrustStore::open(case.directory.path(), [92; 32], case.context)?;
        assert!(reopened.refresh(case.signed.metadata.metadata(), NOW, case.context).is_err());
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(&reopened),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()
    })
}

#[test]
fn expiry_at_final_publication_preserves_old_shell_and_completed_candidate_files() -> TestResult {
    run(1, |case| {
        let mut checks = 0;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || {
                        checks += 1;
                        if checks >= 3 { EXPIRES } else { NOW }
                    },
                    case.context
                )
                .is_err()
        );
        assert_eq!(checks, 3);
        case.preserved()?;
        let candidates: Vec<PathBuf> = std::fs::read_dir(case.directory.path().join("releases"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<_>>()?;
        let candidate = candidates
            .iter()
            .find(|path| path.as_path() != case.original)
            .ok_or("Candidate absent")?;
        assert!(candidate.join("installation-ready.json").is_file());
        assert_eq!(
            std::fs::read(candidate.join("startup.local.toml"))?,
            std::fs::read(case.original.join("startup.local.toml"))?
        );
        Ok(())
    })
}

#[test]
fn current_key_rotation_revokes_download_time_authority() -> TestResult {
    run(1, |case| {
        case.trust.rotate(&root(&key(94)?, 2, Some(&key(92)?))?, case.context)?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()
    })
}

#[test]
fn foreign_installation_with_same_logical_id_and_keys_cannot_authorize_publication() -> TestResult {
    run(1, |case| {
        let other = tempfile::tempdir()?;
        let directory = Directory::open(other.path(), false)?;
        fixture::installed(&directory, case.context)?;
        let foreign = case.signed.initialize(directory.path(), case.context)?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(&foreign),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()
    })
}

#[test]
fn publication_requires_durable_checkpoint_and_cannot_bypass_download_authority() -> TestResult {
    run(1, |case| {
        assert!(case.prepared.activate(case.runtime, case.context).is_err());
        let directory = Directory::open(&case.directory.path().join(".update-trust"), false)?;
        let checkpoint = directory.read("checkpoint")?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()?;
        drop(checkpoint);
        let installed = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || NOW,
            case.context,
        )?;
        assert_ne!(installed.directory(), case.original);
        Ok(())
    })
}

#[test]
fn updater_payload_and_fabricated_engine_capabilities_cannot_reach_application_publication()
-> TestResult {
    run(1, |case| {
        let (manifest, compressed) = test_package::distribution()?;
        let archive = test_package::seal(&manifest, &compressed);
        let mut wrong = SignedPackage::new(&archive, 2)?;
        wrong.request.component = Component::Updater;
        let mut descriptor: serde_json::Value = serde_json::from_slice(&wrong.descriptor)?;
        descriptor["component"] = "updater".into();
        wrong.descriptor = serde_json::to_vec(&descriptor)?;
        wrong.metadata = Set::targets(
            &key(92)?,
            [2; 3],
            &[
                (&wrong.request.target_name(), &wrong.descriptor),
                (&format!("{:x}.ssupdate", Sha256::digest(&archive)), &archive),
            ],
            EXPIRES,
        )?;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &wrong.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        let mut fabricated = SignedPackage::new(&archive, 1)?;
        fabricated.request.updater_release += 1;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &fabricated.authorization(case.trust),
                    || NOW,
                    case.context
                )
                .is_err()
        );
        case.preserved()
    })
}

#[test]
fn trust_writer_remains_held_through_final_check_and_candidate_preparation() -> TestResult {
    run(1, |case| {
        let rotated = root(&key(94)?, 2, Some(&key(92)?))?;
        let directory = Directory::open(&case.directory.path().join(".update-trust"), false)?;
        let writer = directory.read("writer")?;
        let mut checks = 0;
        let installed = case.prepared.prepare_update(
            case.runtime,
            &case.signed.authorization(case.trust),
            || {
                checks += 1;
                assert!(matches!(writer.try_lock().map_err(io::Error::from),
                    Err(cause) if cause.kind() == io::ErrorKind::WouldBlock));
                NOW
            },
            case.context,
        )?;
        assert_eq!(checks, 5);
        assert_eq!(
            entrypoint::selected(case.directory, case.context)?.ok_or("Selection absent")?.path(),
            case.original
        );
        let store =
            crate::setup_storage::SelectionStore::open(case.directory.path(), case.context)?;
        let state = store.load(case.context)?.ok_or("Candidate selection absent")?;
        assert!(matches!(state.candidate(),
            Some(sugarsubstitute_contracts::installation::Candidate::Prepared { generation })
                if generation == installed.generation()));
        writer.try_lock().map_err(io::Error::from)?;
        drop(writer);
        case.trust.rotate(&rotated, case.context)?;
        Ok(())
    })
}

#[test]
fn clock_regression_between_final_checks_cannot_extend_download_authority() -> TestResult {
    run(1, |case| {
        let mut checks = 0;
        assert!(
            case.prepared
                .prepare_update(
                    case.runtime,
                    &case.signed.authorization(case.trust),
                    || {
                        checks += 1;
                        match checks {
                            1 => NOW,
                            2 => NOW + 20,
                            _ => NOW + 10,
                        }
                    },
                    case.context
                )
                .is_err()
        );
        assert_eq!(checks, 3);
        case.preserved()
    })
}

mod candidates;
mod launch;
mod recovery;
mod releases;
mod trials;
