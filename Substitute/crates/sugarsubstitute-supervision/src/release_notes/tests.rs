// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The changelog sections a fetch selects, and a fetch's answers from an actual loopback server.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use crate::update_http::fixture::{Running, complete, exchange};
use base64::Engine;
use sugarsubstitute_contracts::host_release_notes::{MAX_NOTES_UNITS, MAX_RELEASES};

const RESERVATION: u64 = 24 * 1024 * 1024;

/// Compare an observed answer without panicking, so the fixture server is still observed.
fn same<T: PartialEq + std::fmt::Debug + ?Sized>(actual: &T, expected: &T) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("expected {expected:?}, observed {actual:?}").into())
    }
}

const CHANGELOG: &str = "\
## [0.3.1](https://example.test/compare/substitute-v0.3.0...substitute-v0.3.1) (2026-11-12)


### Bug Fixes

* **caption:** keep the update line ([abc1234](https://example.test/commit/abc1234))

# [0.3.0](https://example.test/compare/substitute-v0.2.0...substitute-v0.3.0) (2026-11-10)


### Features

* **updates:** show what is new

## [0.2.1](https://example.test/compare) (2026-11-01)

### Bug Fixes

* a patch

## 0.2.0 (2026-10-20)

* the first release
";

fn versions(selection: &Selection) -> Vec<&str> {
    selection.releases.iter().map(|release| release.version().as_str()).collect()
}

#[test]
fn a_range_selects_every_section_after_since_newest_first_with_its_heading() -> TestResult {
    let selection = changelog::select(CHANGELOG, "0.3.1", "0.2.0").ok_or("Nothing selected")?;
    assert_eq!(versions(&selection), ["0.3.1", "0.3.0", "0.2.1"]);
    assert_eq!(selection.omitted, 0);
    let minor = selection.releases[1].notes();
    assert!(minor.starts_with("# [0.3.0](https://example.test/compare/"), "{minor}");
    assert!(minor.ends_with("* **updates:** show what is new"), "a minor ends at the next patch");
    assert!(!minor.contains("0.2.1"));
    assert_eq!(
        changelog::select(CHANGELOG, "0.2.0", "0.1.0")
            .map(|found| found.releases[0].notes().to_owned()),
        Some("## 0.2.0 (2026-10-20)\n\n* the first release".to_owned()),
        "a heading without a compare link is a section too"
    );
    Ok(())
}

#[test]
fn a_since_not_older_than_the_running_version_selects_the_running_version_alone() -> TestResult {
    for since in ["0.3.1", "0.4.0", "not-a-version"] {
        let selection = changelog::select(CHANGELOG, "0.3.1", since).ok_or("Nothing selected")?;
        assert_eq!(versions(&selection), ["0.3.1"], "{since}");
    }
    Ok(())
}

#[test]
fn a_running_version_without_a_section_is_not_published() {
    assert_eq!(changelog::select(CHANGELOG, "0.4.0", "0.2.0"), None);
    assert_eq!(changelog::select("", "0.3.1", "0.3.1"), None);
    assert_eq!(changelog::select(CHANGELOG, "latest", "0.2.0"), None);
}

#[test]
fn a_long_range_keeps_the_newest_versions_within_the_bounds_and_counts_the_rest() -> TestResult {
    let mut long = String::new();
    for patch in (0..(MAX_RELEASES as u64 + 6)).rev() {
        long.push_str(&format!("## 1.0.{patch} (2026-01-01)\n\n* change {patch}\n\n"));
    }
    let running = format!("1.0.{}", MAX_RELEASES + 5);
    let selection = changelog::select(&long, &running, "0.9.0").ok_or("Nothing selected")?;
    assert_eq!(selection.releases.len(), MAX_RELEASES);
    assert_eq!(selection.releases[0].version().as_str(), running);
    assert_eq!(selection.omitted, 6);

    let paragraph = format!("* {}\n", "x".repeat(4096));
    let huge = format!("## 2.0.0 (2026-01-01)\n\n{}", paragraph.repeat(300));
    let selection = changelog::select(&huge, "2.0.0", "2.0.0").ok_or("Nothing selected")?;
    let notes = selection.releases[0].notes();
    assert!(notes.encode_utf16().count() <= MAX_NOTES_UNITS && notes.len() <= MAX_NOTES_BYTES);
    assert!(notes.ends_with(&"x".repeat(16)), "an oversized section is cut at a line break");
    Ok(())
}

/// A contents API reply carrying `changelog`, wrapped the way GitHub wraps base64 content.
fn contents(changelog: &str) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(changelog);
    let wrapped: Vec<String> =
        encoded.as_bytes().chunks(60).map(|line| String::from_utf8_lossy(line).into()).collect();
    serde_json::json!({"type": "file", "encoding": "base64", "content": wrapped.join("\n")})
        .to_string()
        .into_bytes()
}

const PATH: &str = "repos/Artificial-Sweetener/SugarSubstitute-Rust/contents/Substitute/CHANGELOG.md?ref=substitute-v0.3.1";

#[test]
fn a_fetch_reads_the_changelog_at_the_running_release_tag() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let (server, _) = Running::start(vec![complete(PATH, &contents(CHANGELOG))], context)?;
        let source = ChangelogSource::loopback(server.port())?;
        let mut http = Http::new();
        let fetched = fetch(&mut http, &source, "0.3.1", "0.3.0", context);
        assert!(http.settled());
        server.finish_after((|| {
            let selection = fetched.map_err(|failure| format!("{failure:?}"))?;
            assert_eq!(versions(&selection), ["0.3.1"]);
            Ok(())
        })())
    })
}

#[test]
fn a_missing_release_is_not_published_and_a_server_failure_is_unavailable() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let (server, _) = Running::start(
            vec![
                exchange(PATH, None, 404, "Content-Length: 0\r\n", b""),
                exchange(PATH, None, 503, "Content-Length: 0\r\n", b""),
                complete(PATH, b"{\"encoding\":\"base64\",\"content\":\"!!!\"}"),
                complete(PATH, &contents("# no versions here\n")),
            ],
            context,
        )?;
        let source = ChangelogSource::loopback(server.port())?;
        let mut http = Http::new();
        let mut answers = Vec::new();
        for _ in 0..4 {
            answers.push(fetch(&mut http, &source, "0.3.1", "0.3.0", context).err());
            assert!(http.settled());
        }
        let expected = [
            Some(ReleaseNotesFailure::NotPublished),
            Some(ReleaseNotesFailure::Unavailable),
            Some(ReleaseNotesFailure::Unavailable),
            Some(ReleaseNotesFailure::NotPublished),
        ];
        server.finish_after(same(&answers[..], &expected[..]))
    })
}

#[test]
fn a_reply_over_the_bound_is_refused_without_reading_it_whole() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let declared = format!("Content-Length: {}\r\n", MAX_REPLY_BYTES + 1);
        let mut oversized = exchange(PATH, None, 200, &declared, b"{");
        oversized.await_client_close = true;
        let (server, _) = Running::start(vec![oversized], context)?;
        let source = ChangelogSource::loopback(server.port())?;
        let mut http = Http::new();
        let answer = fetch(&mut http, &source, "0.3.1", "0.3.0", context).err();
        assert!(http.settled());
        server.finish_after(same(&answer, &Some(ReleaseNotesFailure::Unavailable)))
    })
}

#[test]
fn a_silent_server_ends_at_the_idle_bound_as_unavailable() -> TestResult {
    worker::run_reserved(RESERVATION, |context, _| {
        let mut stalled = exchange(PATH, None, 200, "Content-Length: 10\r\n", b"{");
        stalled.await_client_close = true;
        let (server, _) = Running::start(vec![stalled], context)?;
        // The server answers only once the client gives up and closes its socket, so the fetch
        // can end only through its idle bound.
        let source = ChangelogSource::loopback(server.port())?.with_idle(Duration::from_millis(50));
        let mut http = Http::new();
        let answer = fetch(&mut http, &source, "0.3.1", "0.3.0", context).err();
        assert!(http.settled());
        server.finish_after((|| {
            assert_eq!(answer, Some(ReleaseNotesFailure::Unavailable));
            assert_eq!(ChangelogSource::substitute()?.idle, RELEASE_NOTES_IDLE);
            Ok(())
        })())
    })
}

#[test]
fn the_published_source_is_the_release_repositorys_changelog_over_https() -> TestResult {
    let source = ChangelogSource::substitute()?;
    assert_eq!(
        source.address("1.2.3"),
        "https://api.github.com/repos/Artificial-Sweetener/SugarSubstitute-Rust/contents/Substitute/CHANGELOG.md?ref=substitute-v1.2.3"
    );
    assert!(source.policy.endpoint(&source.address("1.2.3")).is_ok());
    assert!(source.policy.endpoint("https://example.test/elsewhere").is_err());
    Ok(())
}
