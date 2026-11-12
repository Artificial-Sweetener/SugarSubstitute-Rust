// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise composed discovery against real sockets, signatures and protected Windows storage.
use super::*;
use crate::{
    setup_storage::test_worker::{self as worker, TestResult, installation},
    update_http::fixture::{Running, complete, exchange},
};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Admission, Channel, Component, Platform},
};
use sugarsubstitute_execution::http_fixture::Exchange;
use sugarsubstitute_update::{Error as Verification, fixture::*};
mod delivery;
mod hostile;
mod resumption;
/// A healthy loopback exchange is never idle for long, so the product's longest supported idle
/// timeout bounds only a dead peer; the server answers at the pace of its events, not a timer.
const IDLE: Duration = Duration::from_secs(120);
const RESERVATION: u64 = RESERVATION_BYTES + 4 * 1024 * 1024;

fn request(channel: Channel) -> Request {
    Request {
        channel,
        component: Component::Application,
        platform: Platform::WindowsX86_64,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Automatic,
    }
}

struct Feed {
    root: Vec<u8>,
    set: Set,
    descriptor: Vec<u8>,
    descriptor_name: String,
    package_name: String,
}
impl Feed {
    /// Distinct role versions expose accidental hardcoded discovery filenames.
    fn new(
        channel: Channel,
        payload: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key = key(96)?;
        let root = root(&key, 1, None)?;
        let digest = format!("{:x}", Sha256::digest(payload));
        let package_name = format!("{digest}.ssupdate");
        let descriptor_name = request(channel).target_name();
        let descriptor = serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "product":"sugarsubstitute", "channel":channel,
            "component":"application", "platform":"windows-x86_64", "sequence":2,
            "version":"2.0.0", "package_sha256":digest, "package_bytes":payload.len(),
            "package_schema":1, "updater_protocol":1, "minimum_updater_release":1,
        }))?;
        let set = Set::targets(
            &key,
            [4, 7, 9],
            &[(&descriptor_name, &descriptor), (&package_name, payload)],
            EXPIRES,
        )?;
        Ok(Self { root, set, descriptor, descriptor_name, package_name })
    }

    fn exchanges(&self, next_root: u32) -> Vec<Exchange> {
        vec![
            exchange(&format!("{next_root}.root.json"), None, 404, "Content-Length: 0\r\n", b""),
            complete("timestamp.json", &self.set.timestamp),
            complete("7.snapshot.json", &self.set.snapshot),
            complete("9.targets.json", &self.set.targets),
            complete(&self.descriptor_name, &self.descriptor),
        ]
    }
}

fn discovered(
    check: Check,
) -> Result<AuthenticatedOffer, Box<dyn std::error::Error + Send + Sync>> {
    match check {
        Check::Offer(offer) => Ok(*offer),
        Check::MoreRoots => Err("Expected an authenticated offer".into()),
    }
}

#[test]
fn release_and_canary_follow_local_subscription_and_signed_role_versions() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        for channel in [Channel::Release, Channel::Canary] {
            let installation = installation(96, context)?;
            let feed = Feed::new(channel, b"archive")?;
            let trust = TrustStore::initialize(installation.path(), [96; 32], &feed.root, context)?;
            let (server, policy) = Running::start(feed.exchanges(2), context)?;
            let mut repository = Repository::new(policy, IDLE);
            let offer =
                discovered(repository.discover(&trust, request(channel), || NOW, context)?)?;
            assert_eq!(offer.offer().admission(), Admission::Prepare);
            assert_eq!(offer.offer().offer().version(), "2.0.0");
            assert_eq!(offer.authorization(&trust).request.channel, channel);
            assert!(repository.settled());
            server.finish()?;
        }
        Ok(())
    })
}

#[test]
fn contiguous_key_rotation_is_saved_before_discovery_uses_new_authority() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(97, context)?;
        let old = key(95)?;
        let new = key(96)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let old_root = root(&old, 1, None)?;
        let mut exchanges = vec![complete("2.root.json", &root(&new, 2, Some(&old))?)];
        exchanges.extend(feed.exchanges(3));
        let trust = TrustStore::initialize(installation.path(), [97; 32], &old_root, context)?;
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        assert_eq!(offer.offer().offer().sequence(), 2);
        assert!(repository.settled());
        drop(offer);
        drop(trust);
        let reopened = TrustStore::open(installation.path(), [97; 32], context)?;
        assert_eq!(reopened.next_root_version(context)?, 3);
        assert!(
            reopened
                .refresh(Set::new(&old, [5, 8, 10], b"revoked", EXPIRES)?.metadata(), NOW, context)
                .is_err()
        );
        server.finish()
    })
}

#[test]
fn long_root_history_yields_durable_continuation_instead_of_skipping_keys() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(98, context)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let key = key(96)?;
        let mut exchanges = Vec::new();
        for version in 2..=33 {
            exchanges.push(complete(&format!("{version}.root.json"), &root(&key, version, None)?));
        }
        exchanges.extend(feed.exchanges(34));
        let trust = TrustStore::initialize(installation.path(), [98; 32], &feed.root, context)?;
        let (server, policy) = Running::start(exchanges, context)?;
        let client = (|| {
            let mut repository = Repository::new(policy, IDLE);
            if !matches!(
                repository.discover(&trust, request(Channel::Release), || NOW, context)?,
                Check::MoreRoots
            ) {
                return Err("Long root history did not yield a continuation".into());
            }
            if !repository.settled() {
                return Err("Root continuation retained HTTP custody".into());
            }
            drop(trust);
            let trust = TrustStore::open(installation.path(), [98; 32], context)?;
            if trust.next_root_version(context)? != 34 {
                return Err("Root continuation did not durably retain every key".into());
            }
            let offer = discovered(repository.discover(
                &trust,
                request(Channel::Release),
                || NOW,
                context,
            )?)?;
            if offer.offer().admission() != Admission::Prepare {
                return Err("Continued discovery did not admit preparation".into());
            }
            Ok(())
        })();
        server.finish_after(client)
    })
}
