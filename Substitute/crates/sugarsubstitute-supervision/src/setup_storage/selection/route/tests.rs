// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Only the generation that should run keeps startup; others redirect once or run accepted only.
use super::*;
use sugarsubstitute_contracts::installation::{
    InstallationId, PackageDigest, PreparationIntent, SignedRelease,
};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn generation(number: u64) -> Result<Generation> {
    Ok(Generation {
        id: GenerationId::new(format!("{number:032x}"))?,
        package: PackageDigest::new(format!("{number:064x}"))?,
        release: SignedRelease::new(format!("1.0.{number}"), number)?,
    })
}

/// Accepted `a`, with `b` staging, prepared or on trial as requested.
fn records() -> Result<[InstallationRecord; 4]> {
    let a = generation(1)?;
    let b = generation(2)?;
    let empty = InstallationRecord::empty(InstallationId::new("9".repeat(32))?);
    let staged = empty.prepare(a.clone(), PreparationIntent::Automatic, 1)?;
    let trial = staged.prepared(&a)?.begin_trial()?;
    let Some(Candidate::Trial { attempt }) = trial.candidate() else {
        return Err("Expected a trial".into());
    };
    let accepted = trial.accept(&attempt.clone())?;
    let staging = accepted.prepare(b.clone(), PreparationIntent::Automatic, 1)?;
    let prepared = staging.prepared(&b)?;
    let on_trial = prepared.begin_trial()?;
    Ok([accepted, staging, prepared, on_trial])
}

#[test]
fn the_selected_generation_keeps_startup_and_others_redirect_to_it_once() -> Result {
    let a = generation(1)?.id;
    let b = generation(2)?.id;
    let [accepted, staging, prepared, on_trial] = records()?;
    for record in [&accepted, &staging] {
        assert_eq!(decide(record, &a, 0)?, Decision::Run(Route::Own));
        assert!(matches!(decide(record, &b, 0)?, Decision::Redirect { target, fallback }
            if target.id == a && fallback == &b));
    }
    for record in [&prepared, &on_trial] {
        assert_eq!(decide(record, &b, 0)?, Decision::Run(Route::Own));
        assert!(matches!(decide(record, &a, 0)?, Decision::Redirect { target, fallback }
            if target.id == b && fallback == &a));
    }
    Ok(())
}

#[test]
fn a_last_hop_engine_never_redirects_and_runs_only_its_accepted_release() -> Result {
    let a = generation(1)?.id;
    let b = generation(2)?.id;
    let stale = generation(3)?.id;
    let [accepted, _, prepared, on_trial] = records()?;
    for record in [&prepared, &on_trial] {
        assert_eq!(decide(record, &a, MAX_ENGINE_HOPS)?, Decision::Run(Route::AcceptedOnly));
    }
    assert!(matches!(decide(&accepted, &stale, MAX_ENGINE_HOPS), Err(RouteFailure::Foreign)));
    assert!(matches!(decide(&prepared, &stale, MAX_ENGINE_HOPS), Err(RouteFailure::Foreign)));
    assert_eq!(decide(&on_trial, &b, MAX_ENGINE_HOPS)?, Decision::Run(Route::Own));
    Ok(())
}

#[test]
fn an_installation_without_an_accepted_release_needs_setup() -> Result {
    let empty = InstallationRecord::empty(InstallationId::new("9".repeat(32))?);
    assert!(matches!(decide(&empty, &generation(1)?.id, 0), Err(RouteFailure::NeedsInstallation)));
    Ok(())
}

#[test]
fn the_previous_release_as_fallback_returns_to_being_accepted() -> Result {
    let a = generation(1)?.id;
    let b = generation(2)?;
    let [_, _, _, on_trial] = records()?;
    let Some(Candidate::Trial { attempt }) = on_trial.candidate() else {
        return Err("Expected a trial".into());
    };
    let upgraded = on_trial.accept(&attempt.clone())?;
    assert_eq!(upgraded.retained().map(|retained| &retained.id), Some(&a));
    assert_eq!(decide(&upgraded, &a, MAX_ENGINE_HOPS)?, Decision::FallBack { failed: &b });
    assert!(matches!(decide(&upgraded, &a, 0)?, Decision::Redirect { target, .. } if target == &b));
    assert_eq!(decide(&upgraded, &b.id, MAX_ENGINE_HOPS)?, Decision::Run(Route::Own));
    Ok(())
}
