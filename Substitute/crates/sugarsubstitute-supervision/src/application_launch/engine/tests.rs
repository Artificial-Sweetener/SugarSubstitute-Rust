// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Every pointer and engine fault ends with a running engine or a typed refusal, never nothing.
use super::*;
use crate::setup_storage::{
    launch_pointer_fixture as fixture,
    test_worker::{self as worker, TestResult, driven::run_driven},
};
use std::os::windows::io::RawHandle;
use sugarsubstitute_contracts::installation::GenerationId;
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObject};

/// Full libtest name of the child-side fixture below.
const FIXTURE: &str = "application_launch::engine::tests::engine_fixture_child";
const MARKER: &str = "engine-fixture:";

/// Child half of the fault fixtures. In an ordinary test run it finds no marker and passes.
/// Started as an engine stand-in it signals its event, waits on it forever, or simply exits.
#[test]
fn engine_fixture_child() -> TestResult {
    let Some(marker) =
        std::env::args().find_map(|argument| argument.strip_prefix(MARKER).map(str::to_owned))
    else {
        return Ok(());
    };
    let (mode, event) = marker.split_once(':').ok_or("Fixture marker is malformed")?;
    let event = HANDLE(event.parse::<usize>()? as RawHandle);
    match mode {
        // SAFETY: the creator passed this inherited event handle for this process alone.
        "signal" => unsafe { SetEvent(event) }?,
        // SAFETY: the event is never signalled by anyone else; the creator terminates this process.
        "hang" => drop(unsafe { WaitForSingleObject(event, INFINITE) }),
        _ => {}
    }
    Ok(())
}

/// Start the test executable as a stand-in child behaving as `mode` towards its one event:
/// `signal` sets it, `hang` waits on it forever, anything else exits at once.
pub(in crate::application_launch) fn stand_in(
    mode: &'static str,
    context: &JobContext,
) -> Result<StartedEngine, ClaimError> {
    let executable =
        std::env::current_exe().map_err(|cause| ClaimError::new("fixture_executable", cause))?;
    let directory =
        executable.parent().ok_or_else(|| ClaimError::invalid("fixture_directory"))?.to_owned();
    super::spawn(&executable, &directory, context, |event| {
        vec![
            "--exact".into(),
            FIXTURE.into(),
            "--nocapture".into(),
            format!("{MARKER}{mode}:{event}").into(),
        ]
    })
}

/// How the fixture pointer's fallback is laid out.
#[derive(Clone, Copy)]
enum Fallback {
    None,
    Present,
    Missing,
}

/// A fixture installation whose pointer names a primary and optionally a fallback engine.
fn installation(
    number: u8,
    fallback: Fallback,
    changed_primary: bool,
    context: &JobContext,
) -> Result<tempfile::TempDir, Box<dyn std::error::Error + Send + Sync>> {
    let root = worker::installation(number, context)?;
    let mut primary =
        fixture::engine(root.path(), &GenerationId::new("1".repeat(32))?, b"one", context)?;
    if changed_primary {
        primary = fixture::changed(&primary);
    }
    let second = match fallback {
        Fallback::None => None,
        Fallback::Present => Some(fixture::engine(
            root.path(),
            &GenerationId::new("2".repeat(32))?,
            b"two",
            context,
        )?),
        Fallback::Missing => Some(fixture::absent(2)?),
    };
    fixture::point(root.path(), &fixture::pointer(primary, second), context)?;
    Ok(root)
}

/// Mode of the stand-in started for the pointer's primary (hop 0) and fallback (hop 1).
fn modes(primary: &'static str, fallback: &'static str) -> impl Fn(u8) -> &'static str {
    move |hops| if hops == 0 { primary } else { fallback }
}

const BYTES: u64 = 4 * crate::setup_storage::POINTER_RESERVATION_BYTES;
const TAKEOVER: std::time::Duration = std::time::Duration::from_secs(10);
/// A stable entry image that does not exist, for tests whose pointer is readable.
const NO_IMAGE: &str = "absent-stable-entry.exe";

#[test]
fn the_primary_engine_takes_over_without_starting_the_fallback() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        let root = installation(180, Fallback::Present, false, context)?;
        let started = std::sync::Mutex::new(Vec::new());
        start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
            crate::registry::lock(&started).push(hops);
            stand_in(modes("signal", "signal")(hops), context)
        })?;
        assert_eq!(*crate::registry::lock(&started), [0]);
        Ok(())
    })
}

#[test]
fn an_engine_that_exits_early_hands_startup_to_the_fallback() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        let root = installation(181, Fallback::Present, false, context)?;
        let started = std::sync::Mutex::new(Vec::new());
        start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
            crate::registry::lock(&started).push(hops);
            stand_in(modes("exit", "signal")(hops), context)
        })?;
        assert_eq!(*crate::registry::lock(&started), [0, 1]);
        Ok(())
    })
}

#[test]
fn a_silent_engine_is_stopped_at_its_bound_and_the_fallback_takes_over() -> TestResult {
    run_driven(
        BYTES,
        |context, handoff| {
            let root = installation(182, Fallback::Present, false, context)?;
            handoff.reach(1);
            start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
                stand_in(modes("hang", "signal")(hops), context)
            })?;
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            let bound = Moment::from_nanos(TAKEOVER.as_nanos() as u64);
            driver.paused(bound)?;
            driver.clock.advance_to(bound)?;
            Ok(())
        },
    )
}

#[test]
fn a_changed_primary_engine_is_never_started_and_the_fallback_runs() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        // Antivirus or a damaged disk changed the primary engine's bytes.
        let root = installation(183, Fallback::Present, true, context)?;
        let started = std::sync::Mutex::new(Vec::new());
        start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
            crate::registry::lock(&started).push(hops);
            stand_in("signal", context)
        })?;
        assert_eq!(*crate::registry::lock(&started), [1]);
        Ok(())
    })
}

/// An antivirus scanner holds the new release's engine while it inspects the update. Startup
/// goes to the previous release, and nothing counts against the update: the pointer is untouched,
/// so once the scanner lets go the next launch starts the new release's engine again.
#[test]
fn a_scanner_holding_the_new_engine_starts_the_previous_release_and_keeps_the_update() -> TestResult
{
    use std::os::windows::fs::OpenOptionsExt;
    worker::run_reserved(BYTES, |context, _| {
        let root = installation(189, Fallback::Present, false, context)?;
        let pointer = crate::setup_storage::read_launch_pointer(root.path(), context)?;
        let engine = root
            .path()
            .join("releases")
            .join(pointer.primary().generation().as_str())
            .join(crate::setup_storage::LAUNCH_ENGINE);
        // Scanners commonly open a file they inspect without sharing it.
        let scanner = std::fs::OpenOptions::new().read(true).share_mode(0).open(&engine)?;
        let started = std::sync::Mutex::new(Vec::new());
        start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
            crate::registry::lock(&started).push(hops);
            stand_in("signal", context)
        })?;
        assert_eq!(*crate::registry::lock(&started), [1]);
        assert_eq!(crate::setup_storage::read_launch_pointer(root.path(), context)?, pointer);
        drop(scanner);
        crate::registry::lock(&started).clear();
        start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
            crate::registry::lock(&started).push(hops);
            stand_in("signal", context)
        })?;
        assert_eq!(*crate::registry::lock(&started), [0]);
        Ok(())
    })
}

/// A scanner's handle on the fixture pointer, opened without sharing as scanners commonly do,
/// shared between the job and the test thread that releases it.
type Scanner = std::sync::Arc<std::sync::Mutex<Option<std::fs::File>>>;

fn hold_pointer(root: &Path, scanner: &Scanner) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let file = root.join("state").join(crate::setup_storage::LAUNCH_POINTER_FILE);
    let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(file)?;
    *crate::registry::lock(scanner) = Some(held);
    Ok(())
}

/// The scanner lets go of the pointer while the root is still waiting for it, so the root
/// starts the primary engine as if nothing had held it.
#[test]
fn a_scanner_releasing_the_pointer_within_the_bound_starts_the_primary_engine() -> TestResult {
    let scanner = Scanner::default();
    let held = scanner.clone();
    run_driven(
        BYTES,
        move |context, handoff| {
            let root = installation(190, Fallback::Present, false, context)?;
            hold_pointer(root.path(), &held)?;
            handoff.reach(1);
            let started = std::sync::Mutex::new(Vec::new());
            start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
                crate::registry::lock(&started).push(hops);
                stand_in("signal", context)
            })?;
            assert_eq!(*crate::registry::lock(&started), [0]);
            assert!(context.now() < Moment::from_nanos(1_000_000_000), "{:?}", context.now());
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            let retries = fixture::hold_retries();
            for retry in &retries[..3] {
                driver.paused(*retry)?;
                driver.clock.advance_to(*retry)?;
            }
            driver.paused(retries[3])?;
            drop(crate::registry::lock(&scanner).take());
            driver.clock.advance_to(retries[3])?;
            Ok(())
        },
    )
}

/// A scanner keeps the pointer past the bound: the root stops waiting at exactly ten seconds of
/// its own clock, starts nothing and reports a storage failure, never "reinstall". Once the
/// scanner lets go, the next launch starts the primary engine.
#[test]
fn a_scanner_holding_the_pointer_past_the_bound_is_reported_and_the_next_launch_starts()
-> TestResult {
    let scanner = Scanner::default();
    let held = scanner.clone();
    run_driven(
        BYTES,
        move |context, handoff| {
            let root = installation(191, Fallback::Present, false, context)?;
            hold_pointer(root.path(), &held)?;
            handoff.reach(1);
            let started = std::sync::Mutex::new(Vec::new());
            let refused = start_pointed_with(
                root.path(),
                Path::new(NO_IMAGE),
                context,
                TAKEOVER,
                |_, hops| {
                    crate::registry::lock(&started).push(hops);
                    stand_in("signal", context)
                },
            );
            assert!(
                matches!(&refused, Err(StartupRefused::Pointer(PointerFailure::Storage(_)))),
                "{refused:?}"
            );
            assert!(crate::registry::lock(&started).is_empty());
            let bound = crate::setup_storage::POINTER_HOLD_BOUND;
            assert_eq!(context.now(), Moment::from_nanos(u64::try_from(bound.as_nanos())?));
            drop(crate::registry::lock(&held).take());
            start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, hops| {
                crate::registry::lock(&started).push(hops);
                stand_in("signal", context)
            })?;
            assert_eq!(*crate::registry::lock(&started), [0]);
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            for retry in fixture::hold_retries() {
                driver.paused(retry)?;
                driver.clock.advance_to(retry)?;
            }
            drop(scanner);
            Ok(())
        },
    )
}
#[test]
fn without_a_working_fallback_every_refusal_is_reported() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        let root = installation(184, Fallback::None, false, context)?;
        let refused =
            start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, _| {
                stand_in("exit", context)
            });
        assert!(
            matches!(&refused, Err(StartupRefused::NoEngine(engines)) if engines.len() == 1),
            "{refused:?}"
        );
        let root = installation(185, Fallback::Missing, false, context)?;
        let refused =
            start_pointed_with(root.path(), Path::new(NO_IMAGE), context, TAKEOVER, |_, _| {
                stand_in("exit", context)
            });
        assert!(
            matches!(&refused, Err(StartupRefused::NoEngine(engines)) if engines.len() == 2),
            "{refused:?}"
        );
        Ok(())
    })
}

/// Write the stable entry's own executable image beside the fixture's releases.
fn stable_entry(root: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let image = root.join("SugarSubstitute.exe");
    std::fs::write(&image, bytes)?;
    Ok(image)
}

/// How a test damages the fixture's pointer.
fn damage(root: &Path, case: &str, context: &JobContext) -> TestResult {
    let file = root.join("state").join(crate::setup_storage::LAUNCH_POINTER_FILE);
    let valid = std::fs::read(&file)?;
    match case {
        "missing" => std::fs::remove_file(&file)?,
        "empty" => fixture::overwrite(root, b"", context)?,
        "truncated" => fixture::overwrite(root, &valid[..valid.len() / 2], context)?,
        "garbled" => {
            let garbled = valid.iter().map(|byte| byte ^ 0x5a).collect::<Vec<_>>();
            fixture::overwrite(root, &garbled, context)?;
        }
        "incompatible" => {
            fixture::overwrite(root, br#"{"schema_version":7,"primary":{}}"#, context)?;
        }
        _ => return Err(format!("Unknown damage {case}").into()),
    }
    Ok(())
}
const DAMAGE: [&str; 5] = ["missing", "empty", "truncated", "garbled", "incompatible"];

/// With the pointer missing or unreadable, the stable entry starts the generation engine that is
/// byte-identical to itself, at hop 0, so that engine routes startup as the record says. The
/// root itself never writes the pointer.
#[test]
fn a_missing_or_damaged_pointer_starts_the_engine_identical_to_the_stable_entry() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        for (number, case) in (192..).zip(DAMAGE) {
            let root = installation(number, Fallback::Present, false, context)?;
            // The stable entry is a copy of the second generation's engine.
            let image = stable_entry(root.path(), b"two")?;
            damage(root.path(), case, context)?;
            let pointer = std::fs::read(
                root.path().join("state").join(crate::setup_storage::LAUNCH_POINTER_FILE),
            )
            .ok();
            let started = std::sync::Mutex::new(Vec::new());
            start_pointed_with(root.path(), &image, context, TAKEOVER, |engine, hops| {
                let generation = engine.directory().file_name().map(|name| name.to_owned());
                crate::registry::lock(&started).push((hops, generation));
                stand_in("signal", context)
            })
            .map_err(|cause| format!("{case}: {cause}"))?;
            assert_eq!(
                *crate::registry::lock(&started),
                [(0, Some(OsString::from("2".repeat(32))))],
                "{case}"
            );
            let after = std::fs::read(
                root.path().join("state").join(crate::setup_storage::LAUNCH_POINTER_FILE),
            )
            .ok();
            assert_eq!(after, pointer, "{case}");
        }
        Ok(())
    })
}

/// Without an engine identical to the stable entry, a missing or damaged pointer is the same
/// typed refusal as before and nothing starts.
#[test]
fn without_an_identical_engine_missing_and_corrupt_pointers_start_nothing() -> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        let never = |_: &PinnedEngine, _: u8| -> Result<StartedEngine, ClaimError> {
            Err(ClaimError::invalid("fixture_unexpected_start"))
        };
        let root = worker::installation(186, context)?;
        let image = stable_entry(root.path(), b"two")?;
        assert!(matches!(
            start_pointed_with(root.path(), &image, context, TAKEOVER, never),
            Err(StartupRefused::Pointer(PointerFailure::Missing))
        ));
        let root = installation(187, Fallback::Present, false, context)?;
        let image = stable_entry(root.path(), b"another release")?;
        fixture::overwrite(root.path(), br#"{"schema_version":1,"primary":"#, context)?;
        assert!(matches!(
            start_pointed_with(root.path(), &image, context, TAKEOVER, never),
            Err(StartupRefused::Pointer(PointerFailure::Damaged))
        ));
        // An identical engine that exits without taking over leaves the pointer's own refusal.
        let image = stable_entry(root.path(), b"one")?;
        assert!(matches!(
            start_pointed_with(root.path(), &image, context, TAKEOVER, |_, _| stand_in(
                "exit", context
            )),
            Err(StartupRefused::Pointer(PointerFailure::Damaged))
        ));
        Ok(())
    })
}

#[test]
fn engine_arguments_are_exact_and_unknown_engine_spellings_are_refused() -> io::Result<()> {
    let valid = |hops: &str| {
        [ENGINE_LAUNCH_ARGUMENT, r"E:\Sugar Substitute", "640", hops].map(OsString::from)
    };
    let (launch, raw) = parse(valid("0"))?.ok_or_else(|| io::Error::other("absent"))?;
    assert_eq!(
        (launch.installation(), launch.hops(), raw),
        (Path::new(r"E:\Sugar Substitute"), 0, 640)
    );
    assert_eq!(parse(valid("1"))?.map(|(launch, _)| launch.hops()), Some(1));
    for invalid in [
        valid("2").to_vec(),
        valid("01").to_vec(),
        [ENGINE_LAUNCH_ARGUMENT, "relative", "640", "0"].map(OsString::from).to_vec(),
        [ENGINE_LAUNCH_ARGUMENT, r"E:\Sugar", "0", "0"].map(OsString::from).to_vec(),
        [ENGINE_LAUNCH_ARGUMENT, r"E:\Sugar", "0640", "0"].map(OsString::from).to_vec(),
        [ENGINE_LAUNCH_ARGUMENT, r"E:\Sugar", "640"].map(OsString::from).to_vec(),
        [ENGINE_LAUNCH_ARGUMENT, r"E:\Sugar", "640", "0", "extra"].map(OsString::from).to_vec(),
        ["--launch-engine-v2", r"E:\Sugar", "640", "0"].map(OsString::from).to_vec(),
    ] {
        assert!(parse(invalid).is_err());
    }
    for unrelated in [vec![], vec![OsString::from("--installed-launch-v3")]] {
        assert!(parse(unrelated)?.is_none());
    }
    Ok(())
}
