// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real HTTP delivery joins protected trust, signed offers and resumed file verification.
use super::*;
use crate::setup_storage::{TRUST_RESERVATION_BYTES, TrustStore};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Channel, Component, Platform, Request},
};
use sugarsubstitute_update::{MetadataSet, fixture::*};

#[test]
fn signed_repository_download_disconnect_reopen_and_resume_reaches_verified_custody() -> TestResult
{
    worker::run_reserved(TRUST_RESERVATION_BYTES + RESERVATION, |context, _| {
        let installation = installation(91, context)?;
        let (manifest, compressed) = crate::setup_storage::test_package::distribution()?;
        let archive = crate::setup_storage::test_package::seal(&manifest, &compressed);
        let payload = archive.as_slice();
        let request = Request {
            channel: Channel::Release,
            component: Component::Application,
            platform: Platform::WindowsX86_64,
            sequence_floor: 0,
            sequence_digest: None,
            updater_release: 1,
            updater_protocol: 1,
            package_schema: 1,
            intent: PreparationIntent::Automatic,
        };
        let digest = format!("{:x}", Sha256::digest(payload));
        let package_name = format!("{digest}.ssupdate");
        let descriptor_name = request.target_name();
        let descriptor = serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "product":"sugarsubstitute", "channel":"release", "component":"application",
            "platform":"windows-x86_64", "sequence":2, "version":"2.0.0", "package_sha256":digest,
            "package_bytes":payload.len(), "package_schema":1, "updater_protocol":1, "minimum_updater_release":1,
        }))?;
        let key = key(91)?;
        let root = root(&key, 1, None)?;
        let set = Set::targets(
            &key,
            [1; 3],
            &[(&descriptor_name, &descriptor), (&package_name, payload)],
            EXPIRES,
        )?;
        let prefix = 7;
        let (server, policy) = Running::start(
            vec![
                complete("timestamp.json", &set.timestamp),
                complete("1.snapshot.json", &set.snapshot),
                complete("1.targets.json", &set.targets),
                complete(&descriptor_name, &descriptor),
                exchange(
                    &package_name,
                    Some("bytes=0-"),
                    200,
                    &format!("Content-Length: {}\r\n", payload.len()),
                    &payload[..prefix],
                ),
                exchange(
                    &package_name,
                    Some("bytes=7-"),
                    206,
                    &format!(
                        "Content-Length: {}\r\nContent-Range: bytes {prefix}-{}/{}\r\n",
                        payload.len() - prefix,
                        payload.len() - 1,
                        payload.len()
                    ),
                    &payload[prefix..],
                ),
            ],
            context,
        )?;
        let mut http = Http::new();
        let mut get = |name: &str| {
            http.metadata(&policy, policy.asset(name)?, MAX_METADATA_BYTES, IDLE, context)
        };
        let timestamp = get("timestamp.json")?;
        let snapshot = get("1.snapshot.json")?;
        let targets = get("1.targets.json")?;
        let descriptor = get(&descriptor_name)?;
        let metadata =
            MetadataSet { timestamp: &timestamp, snapshot: &snapshot, targets: &targets };
        let repository = Sha256::digest(&root).into();
        {
            let store = TrustStore::initialize(installation.path(), repository, &root, context)?;
            let target =
                store.offer(metadata, &request, &descriptor, NOW, context)?.into_package()?;
            let mut download = Download::open(store.runtime_cache(context)?, target, context)?;
            assert!(
                http.package(&policy, policy.asset(&package_name)?, &mut download, IDLE, context)
                    .is_err()
            );
            assert!(http.settled());
            assert_eq!(download.offset()?, prefix as u64);
        }
        let store = TrustStore::open(installation.path(), repository, context)?;
        let metadata =
            MetadataSet { timestamp: &timestamp, snapshot: &snapshot, targets: &targets };
        let target = store.offer(metadata, &request, &descriptor, NOW, context)?.into_package()?;
        let mut download = Download::open(store.runtime_cache(context)?, target, context)?;
        assert_eq!(download.offset()?, prefix as u64);
        http.package(&policy, policy.asset(&package_name)?, &mut download, IDLE, context)?;
        assert!(http.settled());
        let mut verified = download.finish(|| NOW, context)?;
        let mut bytes = Vec::new();
        verified.read_to_end(&mut bytes)?;
        assert_eq!(bytes, payload);
        assert_eq!(verified.identity().digest(), Sha256::digest(payload).as_slice());
        let workspace = tempfile::tempdir()?;
        let bundle = crate::setup_storage::VerifiedBundle::from_download(
            verified,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?;
        assert_eq!(bundle.release(), "local-package");
        assert!(bundle.worker()?.executable().is_file());
        server.finish()
    })
}
