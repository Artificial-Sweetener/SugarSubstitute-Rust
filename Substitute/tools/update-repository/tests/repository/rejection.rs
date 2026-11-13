// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Nothing is signed on top of an unauthenticated state, with an unauthorized key, or into a
//! state the application's verifier would refuse.

use super::support::*;
use std::collections::BTreeMap;
use sugarsubstitute_update_repository::{
    Change, Error, Files, OnlineKeys, Repository, initialize, keys::SigningKey, prepare, verify,
};

struct Keys {
    root: SigningKey,
    targets: SigningKey,
    snapshot: SigningKey,
    timestamp: SigningKey,
}
impl Keys {
    fn new(base: u8) -> Result<Self, Error> {
        Ok(Self {
            root: key(base)?,
            targets: key(base + 1)?,
            snapshot: key(base + 2)?,
            timestamp: key(base + 3)?,
        })
    }
    fn online(&self) -> OnlineKeys<'_> {
        OnlineKeys { targets: &self.targets, snapshot: &self.snapshot, timestamp: &self.timestamp }
    }
}

/// A repository with one release, and the root it was created with.
fn released(keys: &Keys) -> Result<(Files, Vec<u8>), Box<dyn std::error::Error>> {
    let mut files = initialize(&keys.root, &keys.online(), START)?.files;
    let root = files.get("1.root.json").cloned().ok_or("no root")?;
    let release = Release::new(1, "0.2.0")?;
    let (published, _) = prepare(
        &files,
        &root,
        Change::Release { keys: keys.online(), targets: release.targets() },
        &release.payloads(),
        START + DAY,
    )?;
    files.extend(published.files);
    files.extend(published.descriptors);
    Ok((files, root))
}

fn none() -> Memory {
    Memory(BTreeMap::new())
}

fn message(result: Result<impl Sized, Error>) -> String {
    match result {
        Ok(_) => "accepted".into(),
        Err(cause) => format!("{cause} {cause:?}"),
    }
}

#[test]
fn keys_the_root_does_not_authorize_sign_nothing() -> TestResult {
    let keys = Keys::new(40)?;
    let other = Keys::new(50)?;
    let (files, root) = released(&keys)?;
    let now = START + 2 * DAY;
    let refused = |change| message(prepare(&files, &root, change, &none(), now));
    assert!(
        refused(Change::RefreshTimestamp { timestamp: &other.timestamp })
            .contains("timestamp key given is not the one root version 1 authorizes")
    );
    let swapped = OnlineKeys {
        targets: &keys.snapshot,
        snapshot: &keys.snapshot,
        timestamp: &keys.timestamp,
    };
    assert!(refused(Change::Resign(swapped)).contains("targets key given"));
    assert!(
        refused(Change::RotateRoot {
            current_root: &other.root,
            next_root: &other.root,
            keys: keys.online()
        })
        .contains("root key given")
    );
    Ok(())
}

#[test]
fn a_replaced_online_key_stops_working_once_the_next_root_names_its_successor() -> TestResult {
    let keys = Keys::new(60)?;
    let (mut files, root) = released(&keys)?;
    let replacement = key(70)?;
    let online =
        OnlineKeys { targets: &keys.targets, snapshot: &keys.snapshot, timestamp: &replacement };
    let change =
        Change::RotateRoot { current_root: &keys.root, next_root: &keys.root, keys: online };
    let (published, report) = prepare(&files, &root, change, &none(), START + 2 * DAY)?;
    assert_eq!(report.root.0, 2);
    files.extend(published.files);
    let now = START + 3 * DAY;
    assert!(
        message(prepare(
            &files,
            &root,
            Change::RefreshTimestamp { timestamp: &keys.timestamp },
            &none(),
            now
        ))
        .contains("root version 2 authorizes")
    );
    prepare(&files, &root, Change::RefreshTimestamp { timestamp: &replacement }, &none(), now)?;
    Ok(())
}

#[test]
fn a_tampered_or_foreign_state_is_never_signed_on() -> TestResult {
    let keys = Keys::new(80)?;
    let (files, root) = released(&keys)?;
    for name in ["timestamp.json", "2.snapshot.json", "2.targets.json"] {
        let mut tampered = files.clone();
        let bytes = tampered.get_mut(name).ok_or("missing")?;
        let index = bytes.len() / 2;
        bytes[index] ^= 0x01;
        assert_ne!(
            message(Repository::authenticate(&tampered, &root).map(|_| ())),
            "accepted",
            "{name}"
        );
    }
    let mut missing = files.clone();
    missing.remove("2.targets.json");
    assert!(
        message(Repository::authenticate(&missing, &root).map(|_| ()))
            .contains("no 2.targets.json")
    );
    let foreign = initialize(&key(90)?, &Keys::new(91)?.online(), START)?.files;
    let foreign_root = foreign.get("1.root.json").ok_or("no root")?;
    assert!(
        message(Repository::authenticate(&files, foreign_root).map(|_| ()))
            .contains("does not publish the trusted 1.root.json")
    );
    Ok(())
}

#[test]
fn a_rewritten_or_rolled_back_version_is_refused_before_it_is_published() -> TestResult {
    let keys = Keys::new(100)?;
    let (base, root) = released(&keys)?;
    let refresh = |now| -> Result<Files, Box<dyn std::error::Error>> {
        let (published, _) = prepare(
            &base,
            &root,
            Change::RefreshTimestamp { timestamp: &keys.timestamp },
            &none(),
            now,
        )?;
        let mut files = base.clone();
        files.extend(published.files);
        Ok(files)
    };
    let first = refresh(START + 2 * DAY)?;
    let rewritten = refresh(START + 3 * DAY)?;
    assert_ne!(first.get("timestamp.json"), rewritten.get("timestamp.json"));
    assert!(
        message(verify(&root, Some(&first), &rewritten, &none(), START + 3 * DAY))
            .contains("Equivocation")
    );
    assert!(
        message(verify(&root, Some(&first), &base, &none(), START + 3 * DAY)).contains("Rollback")
    );
    verify(&root, Some(&base), &rewritten, &none(), START + 3 * DAY)?;
    Ok(())
}

#[test]
fn releases_sign_only_names_installations_request_and_offers_whose_package_is_signed() -> TestResult
{
    let keys = Keys::new(110)?;
    let (files, root) = released(&keys)?;
    let now = START + 2 * DAY;
    let release = Release::new(2, "0.3.0")?;
    let attempt = |targets| {
        message(prepare(
            &files,
            &root,
            Change::Release { keys: keys.online(), targets },
            &release.payloads(),
            now,
        ))
    };
    assert!(
        attempt(vec![target("0000.ssupdate", &release.package, false)])
            .contains("not named by its SHA-256")
    );
    assert!(
        attempt(vec![target("notes.json", b"{}", true)]).contains("neither a channel descriptor")
    );
    assert!(
        attempt(vec![target(&request().target_name(), &release.descriptor, false)])
            .contains("must carry exactly its descriptor bytes")
    );
    let [_, descriptor] = <[_; 2]>::try_from(release.targets()).map_err(|_| "two targets")?;
    assert!(attempt(vec![descriptor]).contains("Incomplete"));
    let mut wrong = release.payloads();
    wrong.0.insert(release.package_name.clone(), b"other bytes".to_vec());
    assert!(
        message(prepare(
            &files,
            &root,
            Change::Release { keys: keys.online(), targets: release.targets() },
            &wrong,
            now
        ))
        .contains("Digest")
    );
    Ok(())
}

#[test]
fn signed_descriptors_must_be_published_exactly() -> TestResult {
    let keys = Keys::new(120)?;
    let (mut files, root) = released(&keys)?;
    files.insert(request().target_name(), b"{\"altered\":true}".to_vec());
    assert!(message(verify(&root, None, &files, &none(), START + 2 * DAY)).contains("Digest"));
    files.remove(&request().target_name());
    assert!(
        message(verify(&root, None, &files, &none(), START + 2 * DAY))
            .contains("signed but not published")
    );
    Ok(())
}

#[test]
fn keys_are_exactly_sixty_four_hexadecimal_digits() -> TestResult {
    let hex = "0f".repeat(32);
    assert_eq!(
        SigningKey::from_hex(&format!(" {hex}\n"))?.public().key_id(),
        SigningKey::from_seed(&[0x0f; 32])?.public().key_id()
    );
    for text in ["", &"0f".repeat(31), &format!("{}zz", "0f".repeat(31)), &"0f".repeat(33)] {
        assert!(SigningKey::from_hex(text).is_err(), "{text}");
    }
    Ok(())
}
