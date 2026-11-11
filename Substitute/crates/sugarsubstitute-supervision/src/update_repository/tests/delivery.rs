// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Composed download retries preserve owned prefixes and reauthenticate before cache use.
use super::*;
use std::io::Read;

#[test]
fn disconnected_download_reopens_resumes_yields_and_verifies_the_real_archive() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(99, context)?;
        let selection = std::sync::Arc::new(crate::setup_storage::SelectionStore::open(
            installation.path(),
            context,
        )?);
        let run = crate::setup_storage::InstallationRun::acquire(selection.clone(), context)?;
        let (manifest, compressed) = crate::setup_storage::test_package::distribution()?;
        let payload = crate::setup_storage::test_package::seal(&manifest, &compressed);
        let feed = Feed::new(Channel::Release, &payload)?;
        let mut exchanges = feed.exchanges(2);
        exchanges.push(exchange(
            &feed.package_name,
            Some("bytes=0-"),
            200,
            &format!("Content-Length: {}\r\n", payload.len()),
            &payload[..7],
        ));
        exchanges.extend(feed.exchanges(2));
        exchanges.push(exchange(
            &feed.package_name,
            Some("bytes=7-"),
            206,
            &format!("Content-Length: 4\r\nContent-Range: bytes 7-10/{}\r\n", payload.len()),
            &payload[7..11],
        ));
        exchanges.push(exchange(
            &feed.package_name,
            Some("bytes=11-"),
            206,
            &format!(
                "Content-Length: {}\r\nContent-Range: bytes 11-{}/{}\r\n",
                payload.len() - 11,
                payload.len() - 1,
                payload.len()
            ),
            &payload[11..],
        ));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        {
            let trust = TrustStore::initialize(installation.path(), [99; 32], &feed.root, context)?;
            let offer = discovered(repository.discover(
                &trust,
                request(Channel::Release),
                || NOW,
                context,
            )?)?;
            assert!(
                repository.prepare(&offer, &trust, installation.path(), || NOW, context).is_err()
            );
            assert!(repository.settled());
        }
        let trust = TrustStore::open(installation.path(), [99; 32], context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        assert!(
            matches!(repository.prepare(&offer, &trust, installation.path(), || NOW, context)?,
            Preparation::Pending {received:11, total} if total == payload.len() as u64)
        );
        assert!(repository.settled());
        let Preparation::Complete(mut verified) =
            repository.prepare(&offer, &trust, installation.path(), || NOW, context)?
        else {
            return Err("Complete package did not verify".into());
        };
        let mut bytes = Vec::new();
        verified.read_to_end(&mut bytes)?;
        assert_eq!(bytes, payload);
        let workspace = tempfile::tempdir()?;
        let bundle = crate::setup_storage::VerifiedBundle::from_download(
            *verified,
            &crate::setup_storage::cache::RuntimeCache::open(
                workspace.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            context,
        )?;
        assert_eq!(bundle.release(), "local-package");
        drop(bundle);
        assert!(matches!(
            repository.prepare(&offer, &trust, installation.path(), || NOW, context)?,
            Preparation::Complete(_)
        ));
        assert!(repository.settled());
        assert!(
            crate::setup_storage::InstallationRun::acquire(selection.clone(), context).is_err()
        );
        drop(run);
        let _next_run = crate::setup_storage::InstallationRun::acquire(selection, context)?;
        server.finish()
    })
}

#[test]
fn current_offer_expired_authority_and_foreign_installation_never_request_a_package() -> TestResult
{
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(100, context)?;
        let other = super::installation(101, context)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let trust = TrustStore::initialize(installation.path(), [100; 32], &feed.root, context)?;
        let mut exchanges = feed.exchanges(2);
        exchanges.extend(feed.exchanges(2));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let mut current = request(Channel::Release);
        current.sequence_floor = 2;
        current.sequence_digest = Some(Sha256::digest(b"archive").into());
        let offer = discovered(repository.discover(&trust, current, || NOW, context)?)?;
        assert_eq!(offer.offer().admission(), Admission::Current);
        assert!(matches!(
            repository.prepare(&offer, &trust, installation.path(), || NOW, context),
            Err(Failure::Verification(Verification::Admission))
        ));
        drop(offer);
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        assert!(repository.prepare(&offer, &trust, other.path(), || NOW, context).is_err());
        assert!(
            repository.prepare(&offer, &trust, installation.path(), || EXPIRES, context).is_err()
        );
        assert!(repository.settled());
        server.finish()
    })
}
