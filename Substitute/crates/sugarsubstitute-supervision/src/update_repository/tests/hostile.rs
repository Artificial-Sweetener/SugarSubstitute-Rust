// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real failed discovery stops traversal, preserves accepted floors and keeps errors observable.
use super::*;
use crate::setup_storage::TrustFailure;

#[test]
fn extended_http_fixture_still_rejects_excess_request_counts_and_bytes() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        use sugarsubstitute_execution::http_fixture::Server;
        assert!(Server::bind(Vec::new(), context).is_err());
        assert!(Server::bind((0..65).map(|_| complete("x", b"")).collect(), context).is_err());
        assert!(Server::bind(vec![complete("x", &vec![0; 4 * 1024 * 1024])], context).is_err());
        let server = Server::bind((0..64).map(|_| complete("x", b"")).collect(), context)?;
        assert_ne!(server.port()?, 0);
        drop(server);
        Ok(())
    })
}

#[test]
fn unavailable_roots_are_errors_except_for_a_clean_not_found_response() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(102, context)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let trust = TrustStore::initialize(installation.path(), [102; 32], &feed.root, context)?;
        for status in [403, 429, 500, 503] {
            let (server, policy) = Running::start(
                vec![exchange("2.root.json", None, status, "Content-Length: 0\r\n", b"")],
                context,
            )?;
            let mut repository = Repository::new(policy, IDLE);
            assert!(
                matches!(repository.discover(&trust, request(Channel::Release), || NOW, context),
                Err(Failure::Http(update_http::Failure::Policy(sugarsubstitute_update::http::Rejection::Status(actual)))) if actual == status)
            );
            assert!(repository.settled());
            assert_eq!(trust.next_root_version(context)?, 2);
            server.finish()?;
        }
        Ok(())
    })
}

#[test]
fn hostile_roles_stop_before_following_their_unverified_references() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        for role in 1..=4 {
            let installation = installation(103, context)?;
            let feed = Feed::new(Channel::Release, b"archive")?;
            let trust =
                TrustStore::initialize(installation.path(), [103; 32], &feed.root, context)?;
            let mut exchanges = feed.exchanges(2);
            let path = exchanges[role].path.trim_start_matches('/').to_owned();
            exchanges[role] = complete(&path, b"unsigned");
            exchanges.truncate(role + 1);
            let (server, policy) = Running::start(exchanges, context)?;
            let mut repository = Repository::new(policy, IDLE);
            assert!(
                repository.discover(&trust, request(Channel::Release), || NOW, context).is_err()
            );
            assert!(repository.settled());
            server.finish()?;
        }
        Ok(())
    })
}

#[test]
fn failed_snapshot_fetch_retains_signed_reference_across_reopen_and_rejects_replay() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(104, context)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let mut exchanges = feed.exchanges(2);
        exchanges[2] = exchange("7.snapshot.json", None, 503, "Content-Length: 0\r\n", b"");
        exchanges.truncate(3);
        let old = Set::new(&key(96)?, [5, 6, 8], b"old", EXPIRES)?;
        exchanges.push(exchange("2.root.json", None, 404, "Content-Length: 0\r\n", b""));
        exchanges.push(complete("timestamp.json", &old.timestamp));
        let (server, policy) = Running::start(exchanges, context)?;
        let mut repository = Repository::new(policy, IDLE);
        {
            let trust =
                TrustStore::initialize(installation.path(), [104; 32], &feed.root, context)?;
            assert!(
                repository.discover(&trust, request(Channel::Release), || NOW, context).is_err()
            );
            assert!(repository.settled());
        }
        let trust = TrustStore::open(installation.path(), [104; 32], context)?;
        assert!(matches!(
            repository.discover(&trust, request(Channel::Release), || NOW, context),
            Err(Failure::Trust(TrustFailure::Verification(Verification::Rollback)))
        ));
        assert!(repository.settled());
        server.finish()
    })
}

#[test]
fn unauthorized_root_rotation_stops_before_fetching_channel_metadata() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(105, context)?;
        let feed = Feed::new(Channel::Release, b"archive")?;
        let trust = TrustStore::initialize(installation.path(), [105; 32], &feed.root, context)?;
        let forged = root(&key(101)?, 2, None)?;
        let (server, policy) = Running::start(vec![complete("2.root.json", &forged)], context)?;
        let mut repository = Repository::new(policy, IDLE);
        assert!(matches!(
            repository.discover(&trust, request(Channel::Release), || NOW, context),
            Err(Failure::Trust(TrustFailure::Verification(_)))
        ));
        assert_eq!(trust.next_root_version(context)?, 2);
        assert!(repository.settled());
        server.finish()
    })
}

#[test]
fn valid_signatures_cannot_substitute_a_canary_descriptor_for_release() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let installation = installation(106, context)?;
        let mut feed = Feed::new(Channel::Canary, b"archive")?;
        feed.descriptor_name = request(Channel::Release).target_name();
        feed.set = Set::targets(
            &key(96)?,
            [4, 7, 9],
            &[(&feed.descriptor_name, &feed.descriptor), (&feed.package_name, b"archive")],
            EXPIRES,
        )?;
        let trust = TrustStore::initialize(installation.path(), [106; 32], &feed.root, context)?;
        let (server, policy) = Running::start(feed.exchanges(2), context)?;
        let mut repository = Repository::new(policy, IDLE);
        assert!(matches!(
            repository.discover(&trust, request(Channel::Release), || NOW, context),
            Err(Failure::Trust(TrustFailure::Verification(Verification::Offer(
                sugarsubstitute_contracts::update::Rejection::Routing
            ))))
        ));
        assert!(repository.settled());
        server.finish()
    })
}
