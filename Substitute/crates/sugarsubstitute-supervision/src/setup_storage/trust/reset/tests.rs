// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fresh history after a reset starts every route at what the installation already holds.
use super::*;
use sugarsubstitute_contracts::installation::{
    GenerationId, InstallationId, PackageDigest, PreparationIntent, SignedRelease,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn generation(sequence: u64) -> Result<Generation, Box<dyn std::error::Error>> {
    Ok(Generation {
        id: GenerationId::new(format!("{sequence:032x}"))?,
        package: PackageDigest::new(format!("{sequence:064x}"))?,
        release: SignedRelease::new(format!("1.0.{sequence}"), sequence)?,
    })
}

/// The floors `history` holds, as (channel, component, sequence, digest) in encoded form.
fn floors(history: &ReleaseHistory) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let value: serde_json::Value = serde_json::from_slice(&history.encode()?)?;
    Ok(value["floors"].as_array().cloned().unwrap_or_default())
}

fn floor<'a>(
    floors: &'a [serde_json::Value],
    channel: &str,
    component: &str,
) -> Option<&'a serde_json::Value> {
    floors.iter().find(|floor| floor["channel"] == channel && floor["component"] == component)
}

#[test]
fn application_routes_start_at_the_highest_release_the_selection_knows() -> TestResult {
    let mut selection = InstallationRecord::empty(InstallationId::new(format!("{:032x}", 7))?);
    for sequence in [2, 3] {
        selection = selection.prepare(generation(sequence)?, PreparationIntent::Automatic, 1)?;
        selection = selection.prepared(&generation(sequence)?)?.begin_trial()?;
        let Some(Candidate::Trial { attempt }) = selection.candidate().cloned() else {
            return Err("Trial missing".into());
        };
        selection = selection.accept(&attempt)?;
    }
    // Release 5 is prepared but not yet running; 3 runs and 2 is retained.
    selection = selection.prepare(generation(5)?, PreparationIntent::Automatic, 1)?;
    selection = selection.prepared(&generation(5)?)?;
    let mut asked = None;
    let history = starting_history(&selection, None, |generation| {
        asked = Some(generation.clone());
        Some((Channel::Release, [9; 32]))
    })?;
    assert_eq!(asked, Some(generation(5)?), "the highest generation's archive was not looked up");
    let floors = floors(&history)?;
    for channel in ["release", "canary"] {
        let application = floor(&floors, channel, "application").ok_or("Application floor")?;
        assert_eq!(application["sequence"], 5);
    }
    // Only the channel the archive was published on admits just that archive.
    let release = floor(&floors, "release", "application").ok_or("Release floor")?;
    assert_eq!(release["digest"], serde_json::json!(vec![9_u8; 32]));
    let canary = floor(&floors, "canary", "application").ok_or("Canary floor")?;
    assert!(canary.get("digest").is_none());
    Ok(())
}

#[test]
fn updater_routes_start_at_this_builds_engine_release() -> TestResult {
    let selection = InstallationRecord::empty(InstallationId::new(format!("{:032x}", 7))?);
    let floors = floors(&starting_history(&selection, None, |_| None)?)?;
    for channel in ["release", "canary"] {
        let updater = floor(&floors, channel, "updater").ok_or("Updater floor")?;
        assert_eq!(updater["sequence"], ENGINE_RELEASE);
        assert!(updater.get("digest").is_none(), "the set-aside engine's package is not known");
        assert!(floor(&floors, channel, "application").is_none(), "nothing runs yet");
    }
    Ok(())
}

#[test]
fn updater_routes_start_at_the_highest_accepted_updater() -> TestResult {
    let selection = InstallationRecord::empty(InstallationId::new(format!("{:032x}", 7))?);
    let accepted = carried::KeptRelease {
        channel: Channel::Release,
        sequence: ENGINE_RELEASE + 5,
        archive_sha256: [7; 32],
    };
    let floors = floors(&starting_history(&selection, Some(accepted), |_| None)?)?;
    let release = floor(&floors, "release", "updater").ok_or("Release updater floor")?;
    assert_eq!(release["sequence"], ENGINE_RELEASE + 5);
    assert_eq!(release["digest"], serde_json::json!(vec![7_u8; 32]));
    let canary = floor(&floors, "canary", "updater").ok_or("Canary updater floor")?;
    assert_eq!(canary["sequence"], ENGINE_RELEASE + 5);
    assert!(canary.get("digest").is_none(), "only the accepted channel names its archive");
    // An accepted updater older than this build's engine does not lower the floor.
    let older = carried::KeptRelease { sequence: ENGINE_RELEASE - 1, ..accepted };
    let lowered = self::floors(&starting_history(&selection, Some(older), |_| None)?)?;
    let release = floor(&lowered, "release", "updater").ok_or("Release updater floor")?;
    assert_eq!(release["sequence"], ENGINE_RELEASE);
    assert!(release.get("digest").is_none());
    Ok(())
}
