// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Descriptors authored at release, signed and then prepared by an installation that runs the
//! application's own verifier.

use super::support::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use sugarsubstitute_contracts::update::{Channel, Component, Platform};
use sugarsubstitute_update_repository::{
    Change, NewTarget, OnlineKeys, initialize,
    offer::{Route, describe, sequence},
    prepare,
};

const ROUTE: Route = Route {
    channel: Channel::Release,
    platform: Platform::WindowsX86_64,
    component: Component::Application,
};

#[test]
fn sequences_grow_with_every_release_version() -> TestResult {
    let ordered = ["0.0.1", "0.1.0", "0.1.1", "0.2.0", "0.10.0", "1.0.0", "1.0.10", "2.3.4"];
    let sequences =
        ordered.iter().map(|version| sequence(version)).collect::<Result<Vec<_>, _>>()?;
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]), "{sequences:?}");
    assert_eq!(sequence("1.2.3")?, (1 << 40) | (2 << 20) | 3);
    for refused in
        ["0.0.0", "v1.0.0", "1.0", "1.0.0.0", "01.0.0", "1.0.0-beta.1", "1.1048576.0", ""]
    {
        assert!(sequence(refused).is_err(), "{refused} was given a sequence");
    }
    Ok(())
}

/// A sealed package and the two release targets describing it.
struct Described {
    package: Vec<u8>,
    name: String,
    targets: Vec<NewTarget>,
}

fn release(version: &str) -> Result<Described, Box<dyn std::error::Error>> {
    let package = format!("sealed package of {version}").into_bytes();
    let digest = Sha256::digest(&package).into();
    let descriptor = describe(ROUTE, version, 1, digest, package.len() as u64)?;
    assert_eq!(descriptor.name, request().target_name());
    let name = format!("{}.ssupdate", hex(&digest));
    let targets =
        vec![target(&name, &package, false), target(&descriptor.name, &descriptor.bytes, true)];
    Ok(Described { package, name, targets })
}

#[test]
fn a_described_release_is_signed_and_prepared_by_an_installation() -> TestResult {
    let (root, targets, snapshot, timestamp) = (key(31)?, key(32)?, key(33)?, key(34)?);
    let online = || OnlineKeys { targets: &targets, snapshot: &snapshot, timestamp: &timestamp };
    let created = initialize(&root, &online(), START)?;
    let shipped = created.files.get("1.root.json").cloned().ok_or("no root")?;
    let mut files = created.files;
    let mut installation = Installation::new(&shipped)?;
    for (day, version) in [(1, "0.2.0"), (2, "0.2.1"), (3, "0.3.0")] {
        let Described { package, name, targets: release_targets } = release(version)?;
        let payloads = Memory(BTreeMap::from([(name.clone(), package.clone())]));
        let now = START + day * DAY;
        let change = Change::Release { keys: online(), targets: release_targets };
        let (published, report) = prepare(&files, &shipped, change, &payloads, now)?;
        assert_eq!(report.packages, vec![name.clone()]);
        files.extend(published.files);
        files.extend(published.descriptors);
        let packages = BTreeMap::from([(name, package)]);
        assert_eq!(installation.check(&files, &packages, now + 1)?, version);
    }
    Ok(())
}

#[test]
fn a_descriptor_installations_would_not_prepare_is_never_written() {
    let digest = [7; 32];
    assert!(describe(ROUTE, "0.2.0", 1, digest, 0).is_err(), "an empty package was offered");
    assert!(describe(ROUTE, "0.2.0-rc.1", 1, digest, 9).is_err(), "a prerelease was offered");
    let updater = Route { component: Component::Updater, ..ROUTE };
    assert!(
        describe(updater, "0.2.0", 1, digest, 9)
            .is_ok_and(|descriptor| { descriptor.name == "updater-release-windows-x86_64.json" })
    );
}
