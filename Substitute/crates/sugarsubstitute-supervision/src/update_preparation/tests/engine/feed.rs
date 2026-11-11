// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One signed snapshot supplies distinct application and updater routes through loopback HTTP.
use super::*;
use sugarsubstitute_update::fixture::{EXPIRES, Set, key};

/// Signed engine requirement and exact network boundary reached by the host-side scenario.
pub(super) struct Feed {
    pub sequence: u64,
    pub requirement: Option<&'static str>,
    pub completed: usize,
}
impl Default for Feed {
    fn default() -> Self {
        Self { sequence: NEXT_ENGINE, requirement: None, completed: 6 }
    }
}

/// Serve only the expected authenticated references; unexpected downloads fail the fixture.
pub(super) fn campaign(
    pipeline: &mut Pipeline,
    fixture: &Fixture,
    feed: Feed,
) -> Result<(Worker, Running), Failure> {
    let mut application = SignedPackage::new(&fixture.archive, 3)?;
    requires(&mut application, &fixture.archive, "minimum_updater_release", NEXT_ENGINE)?;
    let (archive, _) = updater::archive(CURRENT_UPDATER_CONTRACT)?;
    let mut engine =
        SignedPackage::component(&archive, feed.sequence.try_into()?, Component::Updater)?;
    if let Some(field) = feed.requirement {
        let mut value: serde_json::Value = serde_json::from_slice(&engine.descriptor)?;
        value[field] = if field == "minimum_updater_release" {
            NEXT_ENGINE.into()
        } else {
            (u64::from(sugarsubstitute_update::ENGINE_PROTOCOL) + 1).into()
        };
        engine.descriptor = serde_json::to_vec(&value)?;
    }
    let metadata = Set::targets(
        &key(92)?,
        [(NEXT_ENGINE + 1).try_into()?; 3],
        &[
            (&application.request.target_name(), &application.descriptor),
            (&engine.request.target_name(), &engine.descriptor),
            (&format!("{:x}.ssupdate", Sha256::digest(&fixture.archive)), &fixture.archive),
            (&format!("{:x}.ssupdate", Sha256::digest(&archive)), &archive),
        ],
        EXPIRES,
    )?;
    application.metadata = Set {
        timestamp: metadata.timestamp.clone(),
        snapshot: metadata.snapshot.clone(),
        targets: metadata.targets.clone(),
    };
    engine.metadata = metadata;
    let mut exchanges = discovery(&application)?;
    if feed.completed >= 4 {
        exchanges.extend(discovery(&engine)?);
    }
    if feed.completed >= 5 {
        exchanges.push(exchange(
            &format!("{:x}.ssupdate", Sha256::digest(&archive)),
            Some("bytes=0-"),
            200,
            &format!("Content-Length: {}\r\n", archive.len()),
            &archive,
        ));
    }
    let (server, policy) =
        pipeline.run(8 * 1024 * 1024, move |context| Running::start(exchanges, &context))?;
    Ok((Worker::new(configuration(fixture, policy))?, server))
}

/// Seed historical healthy-engine state through actual publication with the health boundary faked.
pub(super) fn seed_accepted(pipeline: &mut Pipeline, fixture: &Fixture) -> TestResult {
    let installation = fixture.installation.path().to_owned();
    let workspace = fixture.workspace.path().to_owned();
    pipeline.run(
        SIGNED_INSTALLER_RESERVATION_BYTES + UPDATER_STAGING_RESERVATION_BYTES,
        move |context| {
            let (archive, _) = updater::archive(CURRENT_UPDATER_CONTRACT)?;
            let (trust, offer, download) = updater::evidence_version(
                &installation,
                &archive,
                Component::Updater,
                NEXT_ENGINE.try_into()?,
                &context,
            )?;
            let prepared = UpdaterPackage::import(
                &installation,
                trust,
                offer,
                download,
                &workspace,
                || NOW,
                &context,
            )?
            .stage(|| NOW, &context)?;
            updater::accept_prepared(prepared, &context)
        },
    )
}
