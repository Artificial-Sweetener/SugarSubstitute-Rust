// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Section 7 done criteria with a fault at every update write.
//!
//! A full update (prepare, stage, trial, accept, retire) is walked once to list its writes. Then,
//! for every write and every fault class (full disk, denied access, torn write, power cut), a fresh
//! installation runs the same update with that one write failing. Afterwards the version accepted
//! before still starts from the launch pointer and the protected selection, the next launch and
//! update finish the release, and the installation holds exactly the files a clean update leaves.
//! Both the first update of an existing installation and a later update are walked.
use super::*;
use crate::{
    durable_storage::write_port::{
        UpdateFile,
        fake::{self, Fault, Observed, Target},
    },
    setup_storage::{
        GENERATION_RETIREMENT_RESERVATION_BYTES, GenerationTurn, InheritedLaunch,
        LAUNCH_RESERVATION_BYTES, LaunchPointerGenerations, PointerFailure, pin_engine,
        read_launch_pointer, retain_running_generation, retire_generations, test_package::inherit,
    },
};
use std::path::Path;
use sugarsubstitute_contracts::{
    application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT},
    execution::JobContext,
    installation::Generation,
};

const RESERVATION: u64 = SIGNED_INSTALLER_RESERVATION_BYTES
    + LAUNCH_RESERVATION_BYTES
    + GENERATION_RETIREMENT_RESERVATION_BYTES;

/// The part of a full update a failure ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Prepare,
    Stage,
    Trial,
    Accept,
    Shell,
    Retire,
}

fn at<E: Into<Failure>>(step: Step) -> impl FnOnce(E) -> (Step, Failure) {
    move |cause| (step, cause.into())
}

/// A failure with every cause it carries.
fn chain(cause: &(dyn std::error::Error + 'static)) -> String {
    let mut text = cause.to_string();
    let mut source = cause.source();
    while let Some(next) = source {
        text.push_str(&format!(": {next}"));
        source = next.source();
    }
    text
}

fn describe((step, cause): (Step, Failure)) -> Failure {
    format!("{step:?}: {}", chain(cause.as_ref())).into()
}

/// Which update a walk faults.
#[derive(Clone, Copy, Debug)]
enum Walk {
    /// The first update of an existing installation, which also enables protected selection,
    /// update trust and the stable launcher.
    First,
    /// An update of an installation that already runs an accepted release.
    Next,
}

impl Walk {
    fn release(self) -> u32 {
        match self {
            Self::First => 1,
            Self::Next => 2,
        }
    }
}

/// An existing installation whose releases carry the stable launcher, so updates also move the
/// launcher records and the stable entry.
fn existing(context: &JobContext) -> Result<Fixture, Failure> {
    Fixture::with_distribution(
        test_package::distribution_with_launcher(
            Some(CURRENT_CONTRACT),
            Some(CURRENT_LAUNCHER_CONTRACT),
        )?,
        context,
    )
}

/// The installation as `walk`'s update finds it, and the release it accepted before.
fn before(walk: Walk, context: &JobContext) -> Result<(Fixture, Option<Generation>), Failure> {
    let fixture = existing(context)?;
    match walk {
        Walk::First => Ok((fixture, None)),
        Walk::Next => {
            update(&fixture, 1, context).map_err(describe)?;
            let accepted = launch(&fixture, context)?.ok_or("First release not accepted")?;
            Ok((fixture, Some(accepted)))
        }
    }
}

/// One full update to `release`, as launch performs it: prepare and stage the release, start it
/// as a trial in a new copy, accept it at readiness, repair the stable entry, retire old versions.
fn update(fixture: &Fixture, release: u32, context: &JobContext) -> Result<(), (Step, Failure)> {
    let signed = SignedPackage::new(&fixture.archive, release).map_err(at(Step::Prepare))?;
    let prepared = fixture
        .import(signed, context)
        .and_then(|imported| imported.prepare(|| NOW, context))
        .map_err(at(Step::Prepare))?;
    let runtime =
        prepared.maintenance_runtime().ok_or("Runtime absent").map_err(at(Step::Stage))?.clone();
    prepared.stage(context).map_err(at(Step::Stage))?;
    let candidate = prepared.complete(&runtime, || NOW, context).map_err(at(Step::Stage))?;
    let mut recovery = None;
    let transfer = match candidate.transfer(|| NOW, context, &mut recovery) {
        Ok(transfer) => transfer,
        Err(cause) => {
            // The launch owner restores an unstarted trial before it reports the failure.
            if let Some(recovery) = &recovery {
                candidate.prepared().restore_unstarted(recovery, context).ok();
            }
            return Err((Step::Trial, cause.into()));
        }
    };
    let inherited = inherit(transfer.handles()).map_err(at(Step::Trial))?;
    let running = retain_running_generation(
        transfer.directory(),
        Some(InheritedLaunch::Trial(inherited)),
        context,
    )
    .map_err(at(Step::Trial))?
    .ok_or("Started copy found no trial")
    .map_err(at(Step::Trial))?;
    // The launching copy exits once the started copy holds the trial.
    drop((transfer, candidate));
    let trial = running.trial().ok_or("Running copy lost its trial").map_err(at(Step::Accept))?;
    trial.accept(context).map_err(at(Step::Accept))?;
    running.repair_launch_entry(context).map_err(at(Step::Shell))?;
    match retire_generations(&running, &LaunchPointerGenerations, context)
        .map_err(at(Step::Retire))?
    {
        GenerationTurn::Finished(_) => Ok(()),
        turn => Err((Step::Retire, format!("Retirement did not finish: {turn:?}").into())),
    }
}

/// Start the installation as the root launcher and the elected copy do, with no update work:
/// the pointer's engine opens with its recorded bytes, the accepted release starts from the
/// selection record, its stable entry is repaired and old versions are retired. `None` when no
/// release was accepted yet: the existing installation then starts as it did before updating.
fn launch(fixture: &Fixture, context: &JobContext) -> Result<Option<Generation>, Failure> {
    let root = fixture.installation.path();
    match read_launch_pointer(root, context) {
        Ok(pointer) => drop(pin_engine(root, pointer.primary(), context)?),
        Err(PointerFailure::Missing) => {}
        Err(cause) => return Err(cause.into()),
    }
    let record = match SelectionStore::existing(root, context)? {
        Some(store) => store.load(context)?,
        None => None,
    };
    let Some(accepted) = record.as_ref().and_then(|record| record.accepted()).cloned() else {
        fixture.preserved()?;
        return Ok(None);
    };
    let directory = root.join("releases").join(accepted.id.as_str());
    let running =
        retain_running_generation(&directory, None, context)?.ok_or("Accepted release absent")?;
    if running.trial().is_some() || running.generation() != &accepted {
        return Err("The accepted release did not start".into());
    }
    running.repair_launch_entry(context)?;
    retire_generations(&running, &LaunchPointerGenerations, context)?;
    Ok(Some(accepted))
}

/// Every file under `root`, the installation's whole disk use, and the bytes of the files that
/// hold app versions: everything except the small bounded records, whose length varies by a few
/// digits with counters and times. Growth shows as extra files, which the file list catches.
fn footprint(root: &Path) -> Result<(Vec<String>, u64), Failure> {
    let mut names = Vec::new();
    let mut bytes = 0;
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
                continue;
            }
            let path = entry.path();
            let record = path.extension().is_none_or(|extension| {
                ["json", "record", "toml"].iter().any(|known| extension.eq_ignore_ascii_case(known))
            });
            if !record {
                bytes += entry.metadata()?.len();
            }
            names.push(path.strip_prefix(root)?.to_string_lossy().into_owned());
        }
    }
    Ok((names, bytes))
}

/// Replace the names that differ per installation by roles: generation folders, and the package
/// digests download cache entries are named after, since each fixture signs its own archive.
fn by_role(names: &[String], roles: &[(String, &str)]) -> Vec<String> {
    let digest = |component: &str| {
        let hex = component.bytes().take_while(u8::is_ascii_hexdigit).count();
        if hex == 64 { format!("<digest>{}", &component[64..]) } else { component.to_owned() }
    };
    let mut named: Vec<String> = names
        .iter()
        .map(|name| roles.iter().fold(name.clone(), |name, (id, role)| name.replace(id, role)))
        .map(|name| name.split('\\').map(digest).collect::<Vec<_>>().join("\\"))
        .collect();
    named.sort();
    named
}

/// Each generation folder of `fixture` by its role after an update.
fn roles(
    fixture: &Fixture,
    before: Option<&Generation>,
    after: &Generation,
) -> Vec<(String, &'static str)> {
    let mut roles = vec![(after.id.as_str().to_owned(), "updated")];
    if let Some(before) = before {
        roles.push((before.id.as_str().to_owned(), "previous"));
    }
    if let Some(original) = fixture.original.file_name().and_then(|name| name.to_str()) {
        roles.push((original.to_owned(), "original"));
    }
    roles
}

/// The writes of one clean update and the files it leaves, by role.
struct Baseline {
    writes: Vec<Observed>,
    names: Vec<String>,
    bytes: u64,
}

fn baseline(walk: Walk) -> Result<Baseline, Failure> {
    let result = Arc::new(std::sync::Mutex::new(None));
    let observed = result.clone();
    worker::run_reserved(RESERVATION, move |context, _| {
        let (fixture, before) = before(walk, context)?;
        let plan = fake::install(fixture.installation.path())?;
        update(&fixture, walk.release(), context).map_err(describe)?;
        let writes = plan.writes();
        drop(plan);
        let accepted = launch(&fixture, context)?.ok_or("Update not accepted")?;
        let (names, bytes) = footprint(fixture.installation.path())?;
        let names = by_role(&names, &roles(&fixture, before.as_ref(), &accepted));
        *crate::native_claims::lock(&observed) = Some(Baseline { writes, names, bytes });
        Ok(())
    })?;
    Ok(crate::native_claims::lock(&result).take().ok_or("Clean update not observed")?)
}

/// Fail write `index` of `walk`'s update with `fault`, then require the section 7 outcome.
fn faulted(walk: Walk, index: usize, fault: Fault, clean: &Baseline) -> TestResult {
    faulted_at(walk, Target::Nth(index), index, fault, clean)
}

/// Fail `target`, which is write `index` of a clean `walk` update, with `fault`.
fn faulted_at(
    walk: Walk,
    target: Target,
    index: usize,
    fault: Fault,
    clean: &Baseline,
) -> TestResult {
    let expected = clean.writes[index].clone();
    let (names, bytes) = (clean.names.clone(), clean.bytes);
    worker::run_reserved(RESERVATION, move |context, _| {
        let label = format!(
            "{walk:?} update, {fault:?} at write {index} ({:?} {})",
            expected.kind,
            expected.destination.file_name().unwrap_or_default().to_string_lossy()
        );
        let (fixture, before) = before(walk, context)?;
        let plan = fake::install(fixture.installation.path())?;
        plan.arm(target, fault);
        let attempt = update(&fixture, walk.release(), context);
        let (fired, injected) = plan.fired().ok_or_else(|| format!("{label}: never reached"))?;
        assert_eq!((fired.kind, injected), (expected.kind, fault), "{label}");
        assert!(attempt.is_err(), "{label}: the update hid the failure and reported success");
        // The machine starts again: nothing else is armed.
        plan.restore_power();
        // The release accepted before, or the update if it was already accepted, starts.
        let accepted = launch(&fixture, context).map_err(|cause| format!("{label}: {cause}"))?;
        // The next update finishes the release; a launch after it starts the new release.
        if accepted == before {
            update(&fixture, walk.release(), context)
                .map_err(|failure| format!("{label}: retry {}", describe(failure)))?;
        }
        let accepted = launch(&fixture, context)
            .map_err(|cause| format!("{label}: {cause}"))?
            .ok_or_else(|| format!("{label}: the release never started"))?;
        assert_ne!(Some(&accepted), before.as_ref(), "{label}: the release never started");
        // Disk use ends exactly where a clean update leaves it.
        let (after, used) = footprint(fixture.installation.path())?;
        let after = by_role(&after, &roles(&fixture, before.as_ref(), &accepted));
        let extra: Vec<&String> = after.iter().filter(|name| !names.contains(name)).collect();
        let missing: Vec<&String> = names.iter().filter(|name| !after.contains(name)).collect();
        assert!(
            extra.is_empty() && missing.is_empty(),
            "{label}: files differ from a clean update; extra {extra:?}, missing {missing:?}"
        );
        assert!(
            used <= bytes,
            "{label}: {used} version bytes after recovery, {bytes} after a clean update"
        );
        drop(plan);
        Ok(())
    })
}

/// Run `case` for every index below `count` on a few threads. Each case faults its own fresh
/// installation, so the cases share nothing; every failed case is reported.
fn side_by_side(count: usize, case: impl Fn(usize) -> TestResult + Sync) -> TestResult {
    use std::sync::atomic::{AtomicUsize, Ordering};
    const WALKERS: usize = 4;
    let next = AtomicUsize::new(0);
    let failures: Vec<String> = std::thread::scope(|scope| {
        let walkers: Vec<_> = (0..WALKERS.min(count))
            .map(|_| {
                scope.spawn(|| {
                    let mut failures = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::SeqCst);
                        if index >= count {
                            return failures;
                        }
                        if let Err(cause) = case(index) {
                            failures.push(cause.to_string());
                        }
                    }
                })
            })
            .collect();
        walkers
            .into_iter()
            .flat_map(|walker| {
                walker.join().unwrap_or_else(|panic| {
                    vec![
                        panic
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
                            .unwrap_or_else(|| "A faulted update panicked".to_owned()),
                    ]
                })
            })
            .collect()
    });
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n").into()) }
}

/// Walk every write of `walk`'s update with one fault class.
fn walk(walk: Walk, fault: Fault) -> TestResult {
    let clean = baseline(walk)?;
    for kind in [UpdateFile::Selection, UpdateFile::Trust, UpdateFile::Payload, UpdateFile::Receipt]
    {
        assert!(clean.writes.iter().any(|write| write.kind == kind), "{kind:?} never written");
    }
    side_by_side(clean.writes.len(), |index| faulted(walk, index, fault, &clean))
}

#[test]
fn a_full_disk_at_any_first_update_write_keeps_the_installation_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::First, Fault::StorageFull)
}

#[test]
fn denied_access_at_any_first_update_write_keeps_the_installation_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::First, Fault::AccessDenied)
}

#[test]
fn a_torn_first_update_write_keeps_the_installation_and_the_next_run_finishes() -> TestResult {
    walk(Walk::First, Fault::TornWrite)
}

#[test]
fn a_power_cut_at_any_first_update_write_keeps_the_installation_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::First, Fault::PowerCut)
}

#[test]
fn a_full_disk_at_any_update_write_keeps_the_accepted_version_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::Next, Fault::StorageFull)
}

#[test]
fn denied_access_at_any_update_write_keeps_the_accepted_version_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::Next, Fault::AccessDenied)
}

#[test]
fn a_torn_update_write_keeps_the_accepted_version_and_the_next_run_finishes() -> TestResult {
    walk(Walk::Next, Fault::TornWrite)
}

#[test]
fn a_power_cut_at_any_update_write_keeps_the_accepted_version_and_the_next_run_finishes()
-> TestResult {
    walk(Walk::Next, Fault::PowerCut)
}

#[test]
fn every_fault_at_the_launch_pointer_write_keeps_the_installation_starting() -> TestResult {
    let clean = baseline(Walk::First)?;
    let index = clean
        .writes
        .iter()
        .position(|write| write.kind == UpdateFile::LaunchPointer)
        .ok_or("The first update never wrote the launch pointer")?;
    side_by_side(Fault::ALL.len(), |fault| {
        faulted_at(
            Walk::First,
            Target::Named(UpdateFile::LaunchPointer, 0),
            index,
            Fault::ALL[fault],
            &clean,
        )
    })
}
