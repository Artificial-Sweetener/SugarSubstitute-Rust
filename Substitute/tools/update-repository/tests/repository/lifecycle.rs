// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A year and more of scheduled signing, checked every day by an installation that runs the
//! application's own verifier and keeps its trust checkpoint, from the root it shipped with.

use super::support::*;
use std::collections::BTreeMap;
use sugarsubstitute_update_repository::{
    Change, Files, OnlineKeys, Report, initialize, lifetimes, prepare,
};

struct Repository {
    files: Files,
    root: Vec<u8>,
    packages: BTreeMap<String, Vec<u8>>,
}
impl Repository {
    /// Sign, verify and publish one change, as the workflow does.
    fn apply(
        &mut self,
        change: Change<'_>,
        payloads: &Memory,
        now: i64,
    ) -> Result<Report, Box<dyn std::error::Error>> {
        let (published, report) = prepare(&self.files, &self.root, change, payloads, now)?;
        for name in published.files.keys() {
            if name != "timestamp.json" {
                assert!(!self.files.contains_key(name), "{name} would be rewritten");
            }
        }
        self.files.extend(published.files);
        self.files.extend(published.descriptors);
        self.packages.extend(payloads.0.clone());
        Ok(report)
    }
}

#[test]
fn daily_monthly_release_and_yearly_root_signing_keep_every_installation_updating() -> TestResult {
    let (root, next_root) = (key(11)?, key(21)?);
    let (targets, snapshot, timestamp) = (key(12)?, key(13)?, key(14)?);
    let online = || OnlineKeys { targets: &targets, snapshot: &snapshot, timestamp: &timestamp };
    let created = initialize(&root, &online(), START)?;
    let shipped = created.files.get("1.root.json").cloned().ok_or("no root")?;
    let mut repository =
        Repository { files: created.files, root: shipped.clone(), packages: BTreeMap::new() };
    let first = Release::new(1, "0.2.0")?;
    let report = repository.apply(
        Change::Release { keys: online(), targets: first.targets() },
        &first.payloads(),
        START + 60,
    )?;
    assert_eq!(report.offers, vec![request().target_name()]);
    assert_eq!(report.packages, vec![first.package_name.clone()]);
    let mut installation = Installation::new(&shipped)?;
    let mut late = Installation::new(&shipped)?;
    let second = Release::new(2, "0.3.0")?;
    for day in 1..=400 {
        let now = START + day * DAY;
        let report = match day {
            200 => repository.apply(
                Change::Release { keys: online(), targets: second.targets() },
                &second.payloads(),
                now,
            )?,
            330 => repository.apply(
                Change::RotateRoot { current_root: &root, next_root: &next_root, keys: online() },
                &Memory(BTreeMap::new()),
                now,
            )?,
            day if day % 30 == 0 => {
                repository.apply(Change::Resign(online()), &Memory(BTreeMap::new()), now)?
            }
            _ => repository.apply(
                Change::RefreshTimestamp { timestamp: &timestamp },
                &Memory(BTreeMap::new()),
                now,
            )?,
        };
        assert_eq!(report.timestamp.1, now + lifetimes::TIMESTAMP_SECONDS, "day {day}");
        if day % 30 == 0 || day == 200 || day == 330 {
            assert_eq!(report.snapshot.1, now + lifetimes::SNAPSHOT_SECONDS, "day {day}");
            assert_eq!(report.targets.1, now + lifetimes::TARGETS_SECONDS, "day {day}");
        }
        let expected = if day < 200 { "0.2.0" } else { "0.3.0" };
        assert_eq!(installation.check(&repository.files, &repository.packages, now + 1)?, expected);
    }
    assert!(repository.files.contains_key("2.root.json"));
    assert_eq!(repository.files.get("3.root.json"), None);
    // An installation that never checked since it shipped follows the whole chain at once,
    // long after its shipped root expired.
    const { assert!(400 * DAY > lifetimes::ROOT_SECONDS) };
    assert_eq!(
        late.check(&repository.files, &repository.packages, START + 400 * DAY + 2)?,
        "0.3.0"
    );
    Ok(())
}

/// A fresh installation accepts the repository at `now`.
fn accepted(repository: &Repository, shipped: &[u8], now: i64) -> TestResult {
    Installation::new(shipped)?.check(&repository.files, &repository.packages, now)?;
    Ok(())
}

/// A fresh installation refuses the repository at `now` because a role has expired.
fn expired(repository: &Repository, shipped: &[u8], now: i64) -> TestResult {
    let error = Installation::new(shipped)?
        .check(&repository.files, &repository.packages, now)
        .err()
        .ok_or("the check succeeded")?;
    assert!(format!("{error:?}").contains("Expired"), "{error:?}");
    Ok(())
}

#[test]
fn each_role_fails_closed_at_its_lifetime_without_its_scheduled_signing() -> TestResult {
    let (root, targets, snapshot, timestamp) = (key(31)?, key(32)?, key(33)?, key(34)?);
    let online = || OnlineKeys { targets: &targets, snapshot: &snapshot, timestamp: &timestamp };
    let created = initialize(&root, &online(), START)?;
    let shipped = created.files.get("1.root.json").cloned().ok_or("no root")?;
    let mut repository =
        Repository { files: created.files, root: shipped.clone(), packages: BTreeMap::new() };
    let release = Release::new(1, "0.2.0")?;
    let released = START + DAY;
    repository.apply(
        Change::Release { keys: online(), targets: release.targets() },
        &release.payloads(),
        released,
    )?;
    // The timestamp alone: 14 days after it was signed.
    accepted(&repository, &shipped, released + lifetimes::TIMESTAMP_SECONDS - 1)?;
    expired(&repository, &shipped, released + lifetimes::TIMESTAMP_SECONDS)?;
    // Daily timestamps cannot extend snapshot and targets past 90 days without a re-sign.
    let mut now = released;
    while now + DAY < released + lifetimes::SNAPSHOT_SECONDS {
        now += DAY;
        repository.apply(
            Change::RefreshTimestamp { timestamp: &timestamp },
            &Memory(BTreeMap::new()),
            now,
        )?;
    }
    accepted(&repository, &shipped, released + lifetimes::SNAPSHOT_SECONDS - 1)?;
    expired(&repository, &shipped, released + lifetimes::SNAPSHOT_SECONDS)?;
    // Monthly re-signing cannot extend the root past its year.
    let mut now = released;
    while now + 30 * DAY < START + lifetimes::ROOT_SECONDS {
        now += 30 * DAY;
        repository.apply(Change::Resign(online()), &Memory(BTreeMap::new()), now)?;
    }
    accepted(&repository, &shipped, START + lifetimes::ROOT_SECONDS - 1)?;
    expired(&repository, &shipped, START + lifetimes::ROOT_SECONDS)?;
    Ok(())
}
