// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signed installation fixture and host-side observation of finite preparation jobs.
use super::*;
use sugarsubstitute_update::http::Policy;

/// Save a real local preference through an independent settings client's execution owner.
pub(super) fn select_subscription(
    fixture: &Fixture,
    channel: sugarsubstitute_contracts::update::Channel,
) -> TestResult {
    let installation = fixture.installation.path().to_owned();
    crate::setup_storage::test_worker::run_reserved(TRUST_RESERVATION_BYTES, move |context, _| {
        let trust = TrustStore::open(&installation, [92; 32], context)?;
        let current = trust.subscription(context)?;
        trust.select_subscription(current, channel, context)?;
        Ok(())
    })
}

pub(super) fn initialized(pipeline: &mut Pipeline) -> Result<(Fixture, SignedPackage), Failure> {
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, |context| {
        let fixture = Fixture::new(&context)?;
        let signed = SignedPackage::new(&fixture.archive, 1)?;
        drop(InstallationSession::maintain(fixture.installation.path(), &context)?.update_trust(
            [92; 32],
            &signed.root,
            &context,
        )?);
        Ok((fixture, signed))
    })
}

/// Configure cached startup using local paths and no transport capability.
pub(super) fn cached_configuration(fixture: &Fixture) -> CachedConfiguration {
    CachedConfiguration {
        installation: fixture.installation.path().to_owned(),
        workspace: fixture.workspace.path().to_owned(),
    }
}

pub(super) fn configuration(fixture: &Fixture, policy: Policy) -> Configuration {
    Configuration {
        installation: fixture.installation.path().to_owned(),
        workspace: fixture.workspace.path().to_owned(),
        repository: [92; 32],
        policy,
        idle: Duration::from_secs(1),
    }
}

/// Seed health through the shared installer and native run owner, never through request facts.
pub(super) fn accepted(pipeline: &mut Pipeline, fixture: Fixture) -> Result<Fixture, Failure> {
    let fixture = pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
            .prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime missing")?.clone();
        drop(prepared.complete(&runtime, || NOW, &context)?);
        Ok(fixture)
    })?;
    accept_pending(pipeline, &fixture)?;
    Ok(fixture)
}

/// Publish the health result at the native storage boundary; this fixture starts no process.
pub(super) fn accept_pending(pipeline: &mut Pipeline, fixture: &Fixture) -> TestResult {
    let root = fixture.installation.path().to_owned();
    pipeline.run(RESERVATION_BYTES, move |context| {
        crate::setup_storage::test_package::maintenance::accept_pending(&root, &context)
    })
}

/// Sign a remote compatibility requirement while the worker keeps its actual local capabilities.
pub(super) fn requires(
    signed: &mut SignedPackage,
    archive: &[u8],
    field: &str,
    value: u64,
) -> TestResult {
    use sugarsubstitute_update::fixture::{EXPIRES, Set, key};
    let mut descriptor: serde_json::Value = serde_json::from_slice(&signed.descriptor)?;
    descriptor[field] = value.into();
    signed.descriptor = serde_json::to_vec(&descriptor)?;
    signed.metadata = Set::targets(
        &key(92)?,
        [1; 3],
        &[
            (&signed.request.target_name(), &signed.descriptor),
            (&format!("{:x}.ssupdate", Sha256::digest(archive)), archive),
        ],
        EXPIRES,
    )?;
    Ok(())
}

pub(super) fn campaign(
    pipeline: &mut Pipeline,
    fixture: &Fixture,
    signed: SignedPackage,
    package: bool,
) -> Result<(Worker, Running), Failure> {
    let mut exchanges = discovery(&signed)?;
    if package {
        exchanges.push(exchange(
            &format!("{:x}.ssupdate", Sha256::digest(&fixture.archive)),
            Some("bytes=0-"),
            200,
            &format!("Content-Length: {}\r\n", fixture.archive.len()),
            &fixture.archive,
        ));
    }
    let (server, policy) =
        pipeline.run(8 * 1024 * 1024, move |context| Running::start(exchanges, &context))?;
    Ok((Worker::new(configuration(fixture, policy))?, server))
}

pub(super) fn discovery(
    signed: &SignedPackage,
) -> Result<Vec<sugarsubstitute_execution::http_fixture::Exchange>, Failure> {
    let snapshot: serde_json::Value = serde_json::from_slice(&signed.metadata.snapshot)?;
    let targets: serde_json::Value = serde_json::from_slice(&signed.metadata.targets)?;
    let snapshot = snapshot["signed"]["version"].as_u64().ok_or("Snapshot version absent")?;
    let targets = targets["signed"]["version"].as_u64().ok_or("Targets version absent")?;
    Ok(vec![
        exchange("2.root.json", None, 404, "Content-Length: 0\r\n", b""),
        complete("timestamp.json", &signed.metadata.timestamp),
        complete(&format!("{snapshot}.snapshot.json"), &signed.metadata.snapshot),
        complete(&format!("{targets}.targets.json"), &signed.metadata.targets),
        complete(&signed.request.target_name(), &signed.descriptor),
    ])
}

pub(super) fn submit(pipeline: &mut Pipeline, worker: &mut Worker) -> TestResult {
    pipeline.admit_available(|client, scope, id| {
        worker.submit(client, scope, id, Moment::from_nanos(2_000_000_000), || NOW)
    })
}

pub(super) fn step(pipeline: &mut Pipeline, worker: &mut Worker) -> Result<Observation, Failure> {
    submit(pipeline, worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    if let Some(cause) = &observed.failure {
        return Err(Box::new(std::io::Error::other(cause.clone())));
    }
    assert!(matches!(observed.outcome, JobOutcome::Success(())));
    Ok(observed)
}

pub(super) fn retire(pipeline: &mut Pipeline, worker: &mut Worker) -> TestResult {
    worker.cancel();
    assert_eq!(worker.progress(), Progress::Retiring);
    submit(pipeline, worker)?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert!(matches!(observed.outcome, JobOutcome::Success(())));
    assert_eq!(observed.progress, Progress::Closed);
    Ok(())
}

pub(super) fn assert_application_unlocked(
    pipeline: &mut Pipeline,
    fixture: &Fixture,
) -> TestResult {
    let root = fixture.installation.path().to_owned();
    pipeline.run(SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let session = InstallationSession::maintain(&root, &context)?;
        let run =
            InstallationRun::acquire(Arc::new(SelectionStore::open(&root, &context)?), &context)?;
        drop((session, run));
        Ok(())
    })?;
    fixture.preserved()
}

/// Observe independent preparation custody only for fixtures with established update trust.
pub(super) fn assert_unlocked(pipeline: &mut Pipeline, fixture: &Fixture) -> TestResult {
    assert_application_unlocked(pipeline, fixture)?;
    let root = fixture.installation.path().to_owned();
    pipeline.run(TRUST_RESERVATION_BYTES, move |context| {
        let mut trust = TrustStore::open(&root, [92; 32], &context)?;
        trust.acquire_preparation(&context)?;
        Ok(())
    })
}
