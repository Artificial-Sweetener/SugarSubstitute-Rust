// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inert engine archives with genuine signed offers and native download custody.
use crate::setup_storage::{
    self as storage, AuthenticatedOffer, TrustStore, VerifiedDownload, test_package,
    test_package::repository::SignedPackage,
    test_worker::{self as worker, TestResult},
};
use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    path::Path,
};
use sugarsubstitute_contracts::{execution::JobContext, update::Component};
use sugarsubstitute_update::{delivery::Response, fixture::NOW};
/// All dependency bytes are inert: tests verify storage, never execute a fixture binary.
pub(crate) fn archive(contract: &[u8]) -> io::Result<(Vec<u8>, String)> {
    let mut entries = Vec::new();
    let mut compressed = GzEncoder::new(Vec::new(), Compression::fast());
    for (name, bytes) in [
        ("app/sugarsubstitute-updater.exe", b"inert engine".as_slice()),
        ("app/sugarsubstitute_updater.pdb", b"inert symbols".as_slice()),
        ("app/vcruntime140.dll", b"inert runtime".as_slice()),
        ("app/sugarsubstitute-updater.json", contract),
    ] {
        entries.push(serde_json::json!({"path":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes))}));
        compressed.write_all(bytes)?;
    }
    let manifest = serde_json::to_vec(
        &serde_json::json!({"schema_version":1,"release_id":"engine-2","files":entries}),
    )?;
    let digest = format!("{:x}", Sha256::digest(&manifest));
    Ok((test_package::seal(&manifest, &compressed.finish()?), digest))
}

/// Deliver the baseline signed release through actual native download custody.
pub(crate) fn evidence(
    root: &Path,
    bytes: &[u8],
    component: Component,
    context: &JobContext,
) -> Result<
    (TrustStore, AuthenticatedOffer, VerifiedDownload),
    Box<dyn std::error::Error + Send + Sync>,
> {
    evidence_version(root, bytes, component, 2, context)
}

/// Vary authenticated release sequence and role versions for resumption and supersession tests.
pub(crate) fn evidence_version(
    root: &Path,
    bytes: &[u8],
    component: Component,
    version: u32,
    context: &JobContext,
) -> Result<
    (TrustStore, AuthenticatedOffer, VerifiedDownload),
    Box<dyn std::error::Error + Send + Sync>,
> {
    evidence_intent(
        root,
        bytes,
        component,
        version,
        sugarsubstitute_contracts::installation::PreparationIntent::Automatic,
        context,
    )
}

/// Supply local repair intent independently of authenticated repository metadata.
pub(crate) fn evidence_intent(
    root: &Path,
    bytes: &[u8],
    component: Component,
    version: u32,
    intent: sugarsubstitute_contracts::installation::PreparationIntent,
    context: &JobContext,
) -> Result<
    (TrustStore, AuthenticatedOffer, VerifiedDownload),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut signed = SignedPackage::component(bytes, version, component)?;
    signed.request.intent = intent;
    let trust = if root.join(".update-trust").exists() {
        TrustStore::open(root, [92; 32], context)?
    } else {
        TrustStore::initialize(root, [92; 32], &signed.root, context)?
    };
    let offer = AuthenticatedOffer::authenticate(
        &trust,
        signed.request,
        signed.metadata.metadata(),
        &signed.descriptor,
        NOW,
        context,
    )?;
    let target = trust
        .offer(signed.metadata.metadata(), &signed.request, &signed.descriptor, NOW, context)?
        .into_package()?;
    let mut download = storage::Download::open(trust.runtime_cache(context)?, target, context)?;
    download.receive(
        Response { status: 200, length: Some(bytes.len() as u64), range: None, encoding: None },
        &mut io::Cursor::new(bytes),
        context,
    )?;
    Ok((trust, offer, download.finish(|| NOW, context)?))
}

/// Admit source and destination custody together under the real execution boundary.
pub(crate) fn run(body: impl FnOnce(&JobContext) -> TestResult + Send + 'static) -> TestResult {
    worker::run_reserved(
        storage::SIGNED_INSTALLER_RESERVATION_BYTES + storage::UPDATER_STAGING_RESERVATION_BYTES,
        move |context, _| body(context),
    )
}

/// Retain a genuine read pin that prevents atomic updater-journal replacement.
pub(crate) fn pin_journal(root: &Path) -> io::Result<std::fs::File> {
    storage::files::Directory::open(&root.join(".update-trust"), false)?
        .read("updater-selection-v1")
}

/// Seed an already healthy fixture at the process-result boundary. Inert bytes are never run;
/// production signature, exact-trial publication and native custody paths remain exercised.
pub(crate) fn accept_prepared(
    prepared: storage::PreparedUpdater,
    context: &JobContext,
) -> TestResult {
    let mut probe = prepared.reserve_trial(context)?.into_probe(|| NOW, context)?;
    probe.publish_settled(storage::UpdaterProbeDisposition::Accepted, || NOW, context)?;
    Ok(())
}
