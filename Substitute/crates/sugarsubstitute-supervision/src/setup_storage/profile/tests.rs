// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve unproven SDK output and reclaim only released private profile generations.
use super::*;
use crate::setup_storage::{
    cache::CachePolicy,
    test_worker::{self as worker, TestResult, pipeline::Pipeline},
};

#[test]
fn four_mib_profile_scope_survives_eight_mib_verification_and_retirement_jobs() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, cache) = pipeline.run(4 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), CachePolicy::default(), &context)?;
        Ok((root, cache))
    })?;
    let (root, cache, profile) = pipeline.run(8 * 1024 * 1024, move |context| {
        let profile = InstallerProfile::prepare(&cache, &[], &context)?;
        assert!(!profile.recovering());
        assert!(!profile.verified());
        std::fs::write(profile.path().join("native-cache"), b"disposable")?;
        Ok((root, cache, profile))
    })?;
    let (root, cache, mut profile) = pipeline.run(8 * 1024 * 1024, move |context| {
        let mut profile = profile;
        let expected = profile.path();
        profile.verify(&expected, &context)?;
        assert!(profile.verified());
        assert!(profile.verify(&expected, &context).is_err());
        Ok((root, cache, profile))
    })?;
    pipeline.run(8 * 1024 * 1024, move |context| {
        let path = profile.path();
        profile.finish(true, &context)?;
        assert_eq!(cache.maintain(cache::utc_seconds()?, &context)?.removed_entries, 0);
        drop(profile);
        assert_eq!(cache.maintain(cache::utc_seconds()?, &context)?.removed_entries, 1);
        assert!(!path.exists());
        drop((root, cache));
        Ok(())
    })?;
    Ok(())
}

#[test]
fn unproven_profile_survives_host_loss_and_inert_recovery_matches_its_exact_generation()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), CachePolicy::default(), context)?;
        let mut profile = InstallerProfile::prepare(&cache, &[], context)?;
        let path = profile.path();
        std::fs::write(path.join("native-cache"), b"keep until released")?;
        profile.finish(false, context)?;
        drop(profile);
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.unfinished_entries, 1);
        let mut recovered = InstallerProfile::prepare(&cache, &[], context)?;
        assert!(recovered.recovering());
        assert_eq!(recovered.path(), path);
        recovered.verify(&path, context)?;
        recovered.finish(true, context)?;
        drop(recovered);
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.removed_entries, 1);
        assert!(!path.exists());
        Ok(())
    })
}

#[test]
fn redirected_profiles_are_rejected_and_unproven_recovery_does_not_hide_a_fresh_attempt()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), CachePolicy::default(), context)?;
        let profile = InstallerProfile::prepare(&cache, &[], context)?;
        let path = profile.path();
        let key = profile.generation()?;
        drop(profile);
        let mut recovered = InstallerProfile::prepare(&cache, &[], context)?;
        let external = root.path().join("authoritative");
        std::fs::create_dir(&external)?;
        std::fs::write(external.join("document"), b"preserve")?;
        assert!(recovered.verify(&external, context).is_err());
        assert!(!recovered.verified());
        recovered.finish(false, context)?;
        drop(recovered);
        let mut fresh = InstallerProfile::prepare(&cache, &[key], context)?;
        assert!(!fresh.recovering());
        assert_ne!(fresh.path(), path);
        fresh.finish(true, context)?;
        drop(fresh);
        cache.maintain(cache::utc_seconds()?, context)?;
        assert_eq!(std::fs::read(external.join("document"))?, b"preserve");
        assert_eq!(cache.maintain(cache::utc_seconds()?, context)?.unfinished_entries, 1);
        Ok(())
    })
}
