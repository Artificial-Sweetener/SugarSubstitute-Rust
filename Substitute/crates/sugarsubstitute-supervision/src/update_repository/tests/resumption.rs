// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A dropped or slow transfer resumes within its attempt; only a stall or no progress ends it.
use super::*;
use std::{io::Read, time::Duration};

/// Short enough that a deliberately silent peer exercises the real WinHTTP idle timeout.
const STALL_IDLE: Duration = Duration::from_millis(400);

/// Full framing with only `prefix` body bytes; `hold` keeps the connection open and silent.
fn partial(feed: &Feed, payload: &[u8], prefix: usize, hold: bool) -> Exchange {
    let mut response = exchange(
        &feed.package_name,
        Some("bytes=0-"),
        200,
        &format!("Content-Length: {}\r\n", payload.len()),
        &payload[..prefix],
    );
    response.await_client_close = hold;
    response
}

/// The remainder of the package from `offset`, as a valid ranged response.
fn remainder(feed: &Feed, payload: &[u8], offset: usize) -> Exchange {
    exchange(
        &feed.package_name,
        Some(&format!("bytes={offset}-")),
        206,
        &format!(
            "Content-Length: {}\r\nContent-Range: bytes {offset}-{}/{}\r\n",
            payload.len() - offset,
            payload.len() - 1,
            payload.len()
        ),
        &payload[offset..],
    )
}

fn package() -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let (manifest, compressed) = crate::setup_storage::test_package::distribution()?;
    Ok(crate::setup_storage::test_package::seal(&manifest, &compressed))
}

#[test]
fn dropped_connection_resumes_within_the_attempt_without_rediscovery() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(110, context)?;
        let payload = package()?;
        let feed = Feed::new(Channel::Release, &payload)?;
        let mut exchanges = feed.exchanges(2);
        exchanges.push(partial(&feed, &payload, 7, false));
        exchanges.push(remainder(&feed, &payload, 7));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let trust = TrustStore::initialize(installation.path(), [110; 32], &feed.root, context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        let total = payload.len() as u64;
        assert!(matches!(
            repository.prepare(&offer, &trust, installation.path(), || NOW, context)?,
            Preparation::Interrupted { received: 7, total: actual, .. } if actual == total
        ));
        assert!(repository.settled());
        let Preparation::Complete(mut verified) =
            repository.prepare(&offer, &trust, installation.path(), || NOW, context)?
        else {
            return Err("Resumed package did not complete".into());
        };
        let mut bytes = Vec::new();
        verified.read_to_end(&mut bytes)?;
        assert_eq!(bytes, payload);
        drop(verified);
        server.finish()
    })
}

#[test]
fn a_drop_that_delivers_nothing_ends_the_attempt() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(111, context)?;
        let payload = package()?;
        let feed = Feed::new(Channel::Release, &payload)?;
        let mut exchanges = feed.exchanges(2);
        exchanges.push(partial(&feed, &payload, 0, false));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let trust = TrustStore::initialize(installation.path(), [111; 32], &feed.root, context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        assert!(repository.prepare(&offer, &trust, installation.path(), || NOW, context).is_err());
        assert!(repository.settled());
        server.finish()
    })
}

#[test]
fn a_stalled_transfer_fails_by_the_idle_timeout_and_keeps_its_prefix() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(112, context)?;
        let payload = package()?;
        let feed = Feed::new(Channel::Release, &payload)?;
        let mut exchanges = feed.exchanges(2);
        exchanges.push(partial(&feed, &payload, 7, true));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, STALL_IDLE);
        let trust = TrustStore::initialize(installation.path(), [112; 32], &feed.root, context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        // The peer goes silent after seven bytes; progress does not excuse the stall.
        let result = repository.prepare(&offer, &trust, installation.path(), || NOW, context);
        assert!(result.is_err(), "A stalled transfer was resumed instead of failing");
        assert!(repository.settled());
        let saved = worker::cache_file(
            installation.path(),
            crate::setup_storage::cache::Namespace::UpdateDownloads,
            "payload",
        )?;
        let saved = std::fs::metadata(saved)?.len();
        assert_eq!(saved, 7);
        server.finish()
    })
}

#[test]
fn a_slow_transfer_reaching_its_job_deadline_continues_from_its_saved_prefix() -> TestResult {
    let payload = package()?;
    let feed = Feed::new(Channel::Release, &payload)?;
    let root = feed.root.clone();
    let total = payload.len() as u64;
    let mut slow = feed.exchanges(2);
    // Still delivering when the step's deadline arrives: the peer is slow, not silent.
    slow.push(partial(&feed, &payload, 7, true));
    let mut rest = feed.exchanges(2);
    rest.push(remainder(&feed, &payload, 7));
    let kept = std::sync::Arc::new(std::sync::Mutex::new(None));
    let retained = kept.clone();
    worker::run_reserved_expiring(RESERVATION, move |context, clock| {
        let installation = installation(113, context)?;
        let anchor = installation.path().to_owned();
        *crate::registry::lock(&retained) = Some(installation);
        let observed = anchor.clone();
        clock.observe_checks(move || {
            crate::setup_storage::test_worker::cache_file(
                &observed,
                crate::setup_storage::cache::Namespace::UpdateDownloads,
                "payload",
            )
            .and_then(std::fs::metadata)
            .is_ok_and(|saved| saved.len() >= 7)
        })?;
        let (server, policy) = Running::start(slow, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let trust = TrustStore::initialize(&anchor, [113; 32], &root, context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        assert!(matches!(
            repository.prepare(&offer, &trust, &anchor, || NOW, context)?,
            Preparation::Interrupted { received: 7, total: actual, .. } if actual == total
        ));
        assert!(repository.settled());
        server.finish()
    })?;
    // A fresh step continues from the saved prefix and completes the same authenticated bytes.
    let installation = crate::registry::lock(&kept).take().ok_or("Installation fixture lost")?;
    let anchor = installation.path().to_owned();
    worker::run_reserved(RESERVATION, move |context, _| {
        let (server, policy) = Running::start(rest, context)?;
        let mut repository = Repository::new(policy, IDLE);
        let trust = TrustStore::open(&anchor, [113; 32], context)?;
        let offer =
            discovered(repository.discover(&trust, request(Channel::Release), || NOW, context)?)?;
        let Preparation::Complete(mut verified) =
            repository.prepare(&offer, &trust, &anchor, || NOW, context)?
        else {
            return Err("Continued package did not complete".into());
        };
        let mut bytes = Vec::new();
        verified.read_to_end(&mut bytes)?;
        assert_eq!(bytes, payload);
        drop(verified);
        drop(trust);
        server.finish()
    })?;
    drop(installation);
    Ok(())
}
