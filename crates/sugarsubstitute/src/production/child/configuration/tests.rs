// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test exact input projection; borrowed wire shape creates no native authority.
use super::*;
use std::{
    num::NonZeroU16,
    os::windows::ffi::OsStrExt,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, Presentation, Theme},
    coordinator_entry::{EntryArgumentsView, EntryEnvironmentView, EntryFrontendView},
    execution::{
        Clock, CompletionWake, ExecutionError, JobCompletion, JobOutcome, ScopeId, ScopeSpec,
        TerminalKind, WaitFuture,
    },
};
type TestResult = Result<(), Box<dyn Error + Send + Sync>>;

struct Release(Arc<AtomicU64>);
impl sugarsubstitute_contracts::execution::MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

fn bytes(units: impl IntoIterator<Item = u16>) -> Vec<u8> {
    units.into_iter().flat_map(u16::to_le_bytes).collect()
}
fn path_view(source: &[u8]) -> Result<EntryWindowsPathView<'_>, Box<dyn Error + Send + Sync>> {
    Ok(EntryWindowsPathView::from_le_bytes(source)?)
}
fn view<'a>(
    connection: EntryConnectionView<'a>,
    directory: EntryWindowsPathView<'a>,
) -> Result<EntryLaunchRequestView<'a>, Box<dyn Error + Send + Sync>> {
    Ok(EntryLaunchRequestView::new(
        connection,
        EntryFrontendView::new(directory, directory, directory, directory),
        directory,
        "Profile_1",
        Presentation::new("ja", Theme::Light, true)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?)
}

#[test]
fn attached_projection_preserves_explicit_origin_profile_and_presentation() -> TestResult {
    let directory = bytes("E:\\profile".encode_utf16());
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let projected = project(view(EntryConnectionView::Attached(origin), path_view(&directory)?)?)?;
    assert_eq!(projected.connection(), &ConnectionConfiguration::Attach(origin));
    let parent =
        crate::production::entry_activation::ProductionActivation::projected_target_for_test(view(
            EntryConnectionView::Attached(origin),
            path_view(&directory)?,
        )?);
    assert!(
        matches!(parent,Ok(sugarsubstitute_contracts::lifecycle::BackendTarget::Attached(actual)) if actual==origin)
    );
    assert_eq!(projected.profile().as_str(), "Profile_1");
    assert_eq!(projected.presentation(), Presentation::new("ja", Theme::Light, true)?);
    Ok(())
}

#[test]
fn managed_projection_preserves_raw_windows_units_and_exact_unicode_arguments() -> TestResult {
    let mut root_units: Vec<u16> = "E:\\Comfy".encode_utf16().collect();
    root_units.extend("日本語".encode_utf16());
    let root = bytes(root_units.iter().copied());
    let mut interpreter_units: Vec<u16> = "E:\\Comfy\\python.exe".encode_utf16().collect();
    interpreter_units.push(0xd800);
    let interpreter = bytes(interpreter_units.iter().copied());
    let argument = "--output-directory=日本語 🧁";
    let mut arguments = Vec::new();
    arguments.extend_from_slice(&u32::try_from(argument.len())?.to_le_bytes());
    arguments.extend_from_slice(argument.as_bytes());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&arguments, 1)?,
        port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
    };
    let projected = project(view(requested, path_view(&interpreter)?)?)?;
    let ConnectionConfiguration::Launch(launch) = projected.connection() else {
        return Err("Managed input changed connection mode".into());
    };
    assert_eq!(launch.root().as_os_str().encode_wide().collect::<Vec<_>>(), root_units);
    assert_eq!(
        launch.interpreter().as_os_str().encode_wide().collect::<Vec<_>>(),
        interpreter_units
    );
    assert_eq!(
        &launch.arguments()[..7],
        [
            "-u",
            "E:\\Comfy日本語\\main.py",
            "--listen",
            "127.0.0.1",
            "--port",
            "8188",
            "--disable-auto-launch"
        ]
    );
    assert_eq!(&launch.arguments()[7..], &[argument]);
    assert_eq!(launch.port().get(), 8188);
    let parent =
        crate::production::entry_activation::ProductionActivation::projected_target_for_test(
            view(requested, path_view(&interpreter)?)?,
        )?;
    assert_eq!(
        parent,
        sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(launch.clone())
    );
    assert_eq!(&launch.arguments()[7..], [argument]);
    Ok(())
}

#[test]
fn borrowed_shape_does_not_bypass_authoritative_relative_path_rejection() -> TestResult {
    let root = bytes("relative".encode_utf16());
    let interpreter = bytes("E:\\python.exe".encode_utf16());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&[], 0)?,
        port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
    };
    assert!(matches!(
        project(view(requested, path_view(&interpreter)?)?),
        Err(ProjectionError::Configuration(_))
    ));
    Ok(())
}

#[test]
fn main_input_keeps_exact_windows_path_and_original_charge_until_drop() -> TestResult {
    let mut units: Vec<u16> = "E:\\profile".encode_utf16().collect();
    units.push(0xd800);
    let directory = bytes(units.iter().copied());
    let released = Arc::new(AtomicU64::new(0));
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(Release(released.clone())));
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let main = project_main(
        view(EntryConnectionView::Attached(origin), path_view(&directory)?)?,
        &memory,
    )?;
    assert_eq!(main.directory().as_os_str().encode_wide().collect::<Vec<_>>(), units);
    assert_eq!(main.profile().as_str(), "Profile_1");
    drop(memory);
    assert_eq!(released.load(Ordering::Acquire), 0);
    drop(main);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}

#[test]
fn exhausted_original_claim_rejects_main_projection_without_changing_input() -> TestResult {
    let directory = bytes("E:\\profile".encode_utf16());
    let released = Arc::new(AtomicU64::new(0));
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(Release(released.clone())));
    let occupied = memory.claim(RESERVATION_BYTES - MAIN_INPUT_BYTES + 1)?;
    let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
    let original = view(EntryConnectionView::Attached(origin), path_view(&directory)?)?;
    assert!(matches!(project_main(original, &memory), Err(ProjectionError::Memory(_))));
    assert_eq!(
        original.profile_directory().units().collect::<Vec<_>>(),
        "E:\\profile".encode_utf16().collect::<Vec<_>>()
    );
    drop(memory);
    assert_eq!(released.load(Ordering::Acquire), 0);
    drop(occupied);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}

/// The external clock stays frozen while the real executor owns all work and scopes.
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Coalesce actual wake revisions without introducing another scheduler or channel.
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Configuration test observer poisoned")),
            )
        })?;
        *revision = revision.checked_add(1).ok_or_else(|| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Configuration test observer exhausted")),
            )
        })?;
        self.changed.notify_all();
        Ok(())
    }
}
/// Share the same real execution boundary for these two finite preparation owners.
pub(in crate::production::child) struct Rig {
    pub(in crate::production::child) owner: sugarsubstitute_execution::ExecutionOwner,
    pub(in crate::production::child) client: ExecutionClient,
    pub(in crate::production::child) scope: Scope,
    wake: Arc<Wake>,
}
impl Rig {
    /// Start one original execution owner, with only clock/wake faked externally.
    pub(in crate::production::child) fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let wake = Arc::new(Wake::default());
        let owner =
            sugarsubstitute_execution::ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { owner, client, scope, wake })
    }
    /// Await the actual matching terminal notice and retained typed completion.
    pub(in crate::production::child) fn completion<T: Send + 'static, E: Send + 'static>(
        &self,
        job: &mut OwnedJob<T, E>,
    ) -> Result<JobCompletion<T, E>, Box<dyn Error + Send + Sync>> {
        let mut noticed = false;
        let until = std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(10))
            .ok_or("Configuration test observation cutoff overflow")?;
        loop {
            let before = *self.wake.revision.lock().map_err(|_| "Wake poisoned")?;
            let batch = self.client.drain_completions(std::num::NonZeroUsize::MIN)?;
            for notice in batch.notifications {
                if (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id) {
                    noticed = true;
                }
            }
            if noticed && let Some(completed) = job.try_take_completion() {
                return Ok(completed);
            }
            let revision = self.wake.revision.lock().map_err(|_| "Wake poisoned")?;
            let remaining = until
                .checked_duration_since(std::time::Instant::now())
                .ok_or("Actual worker completion absent")?;
            let (next, timeout) = self
                .wake
                .changed
                .wait_timeout_while(revision, remaining, |revision| *revision == before)
                .map_err(|_| "Wake wait poisoned")?;
            if timeout.timed_out() && *next == before {
                return Err("Actual worker completion absent".into());
            }
        }
    }
}
/// Construct only inert requested data using the actual worker's original memory.
pub(in crate::production::child) fn main_input(
    context: &JobContext,
) -> Result<PreparedMainInput, Box<dyn Error + Send + Sync>> {
    let directory: Vec<_> = "E:\\profile".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let path = EntryWindowsPathView::from_le_bytes(&directory)?;
    let requested = EntryLaunchRequestView::new(
        EntryConnectionView::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
        EntryFrontendView::new(path, path, path, path),
        path,
        "Profile_1",
        Presentation::new("ja", Theme::Light, true)?,
        EntryEnvironmentView::from_le_bytes(&[0, 0, 0, 0])?,
    )?;
    Ok(project_main(requested, context.memory())?)
}

/// The original invalid combination rejects synchronously; the same ID then executes conversion.
#[test]
fn finite_projection_spec_rejects_startup_then_admits_original_lifecycle_conversion() -> TestResult
{
    let rig = Rig::new()?;
    let original = Moment::from_nanos(100);
    let spec = request(JobId::new(1)?, original);
    let entered = Arc::new(AtomicU64::new(0));
    let rejected_calls = entered.clone();
    let rejected = rig.client.submit_blocking(
        &rig.scope,
        JobSpec { operation: Operation::Startup, ..spec },
        move |_| {
            rejected_calls.fetch_add(1, Ordering::AcqRel);
            Ok::<_, ProjectionError>(())
        },
    );
    assert!(matches!(
        rejected,
        Err(AdmissionError::WrongClass {
            operation: Operation::Startup,
            class: JobClass::InfrastructureLifecycle
        })
    ));
    assert_eq!(entered.load(Ordering::Acquire), 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    let mut job = rig.client.submit_blocking(&rig.scope, spec, move |context| {
        checkpoint(&context)?;
        let claim = context.memory().claim(PROJECTION_BYTES).map_err(ProjectionError::Memory)?;
        let directory = bytes("E:\\profile".encode_utf16());
        let origin = LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?;
        let requested = view(EntryConnectionView::Attached(origin), path_view(&directory)?)?;
        let configuration = project(requested)?;
        let main = project_main(requested, context.memory())?;
        checkpoint(&context)?;
        Ok::<_, Box<dyn Error + Send + Sync>>((configuration, main, claim))
    })?;
    let completed = rig.completion(&mut job)?;
    assert_eq!(completed.spec, spec);
    assert_eq!(completed.spec.deadline, Some(original));
    assert_eq!(completed.outcome.kind(), TerminalKind::Success);
    assert_eq!(completed.memory.bytes(), RESERVATION_BYTES);
    let JobOutcome::Success((configuration, main, claim)) = completed.outcome else {
        return Err("Conversion failed".into());
    };
    assert_eq!(configuration.profile().as_str(), "Profile_1");
    assert_eq!(configuration.presentation(), Presentation::new("ja", Theme::Light, true)?);
    assert_eq!(main.directory(), Path::new("E:\\profile"));
    drop((completed.memory, job));
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    drop((configuration, main, claim));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Both actual projections reject protected overrides while keeping the original borrowed bytes.
#[test]
fn managed_parent_and_child_reject_protected_extras_without_rewriting_source() -> TestResult {
    let root = bytes("E:\\Comfy".encode_utf16());
    let interpreter = bytes("E:\\Comfy\\python.exe".encode_utf16());
    for extra in ["--listen=0.0.0.0", "--port=9999", "--auto-launch", "--front-end-root=private"] {
        let mut raw = Vec::new();
        raw.extend_from_slice(&u32::try_from(extra.len())?.to_le_bytes());
        raw.extend_from_slice(extra.as_bytes());
        let unchanged = raw.clone();
        let requested = EntryConnectionView::Managed {
            root: path_view(&root)?,
            interpreter: path_view(&interpreter)?,
            arguments: EntryArgumentsView::from_encoded(&raw, 1)?,
            port: NonZeroU16::new(8188).ok_or("Invalid test port")?,
        };
        let original = view(requested, path_view(&interpreter)?)?;
        assert!(matches!(
            project(original),
            Err(ProjectionError::ManagedLaunch(
                sugarsubstitute_comfy::launch::ManagedLaunchError::ProtectedOption
            ))
        ));
        let parent =
            crate::production::entry_activation::ProductionActivation::projected_target_for_test(
                original,
            )
            .err()
            .ok_or("Parent admitted a protected option")?;
        assert!(matches!(
            parent.downcast_ref::<sugarsubstitute_comfy::launch::ManagedLaunchError>(),
            Some(sugarsubstitute_comfy::launch::ManagedLaunchError::ProtectedOption)
        ));
        assert_eq!(raw, unchanged);
    }
    Ok(())
}

/// A root that cannot form the String script argument fails closed instead of changing raw units.
#[test]
fn non_unicode_managed_script_root_keeps_original_units_on_projection_rejection() -> TestResult {
    let units = [69, 58, 92, 0xd800];
    let root = bytes(units);
    let interpreter = bytes("E:\\python.exe".encode_utf16());
    let requested = EntryConnectionView::Managed {
        root: path_view(&root)?,
        interpreter: path_view(&interpreter)?,
        arguments: EntryArgumentsView::from_encoded(&[], 0)?,
        port: NonZeroU16::new(8188).ok_or("Invalid port")?,
    };
    let original = view(requested, path_view(&interpreter)?)?;
    assert!(matches!(
        project(original),
        Err(ProjectionError::ManagedLaunch(
            sugarsubstitute_comfy::launch::ManagedLaunchError::ArgumentShape
        ))
    ));
    assert!(
        crate::production::entry_activation::ProductionActivation::projected_target_for_test(
            original
        )
        .is_err()
    );
    assert_eq!(path_view(&root)?.units().collect::<Vec<_>>(), units);
    Ok(())
}

#[test]
fn acquisition_retry_failed_main_input_transfer_preserves_both_original_claims_and_slots()
-> TestResult {
    let rig = Rig::new()?;
    let original = Moment::from_nanos(100);
    let mut job =
        rig.client.submit_blocking(&rig.scope, request(JobId::new(1)?, original), |context| {
            let current = main_input(&context)?;
            let donor = main_input(&context)?;
            let allocation = context.memory().claim(PROJECTION_BYTES)?;
            let configuration = StartupConfiguration::new(
                ConnectionConfiguration::Attach(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                ProfileId::new("Profile_1".into())?,
                Presentation::new("ja", Theme::Light, true)?,
            );
            Ok::<_, Box<dyn Error + Send + Sync>>((
                PreparedChildConfiguration {
                    configuration,
                    main: current,
                    source: None,
                    _allocation: allocation,
                },
                Some(donor),
            ))
        })?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success((configuration, mut donor)) = completion.outcome else {
        return Err("Actual owned input construction failed".into());
    };
    let mut current = configuration.into_startup((LaunchId::new(1)?, Generation::new(1)?));
    let before = rig.scope.snapshot().reserved_bytes;
    assert!(current.restore_main_input(&mut donor).is_err());
    assert!(current.main.is_some() && donor.is_some());
    assert_eq!(current.session.launch(), LaunchId::new(1)?);
    assert_eq!(rig.scope.snapshot().reserved_bytes, before);
    assert_eq!(job.spec().deadline, Some(original));
    let held = current.take_main_input();
    current.restore_main_input(&mut donor).map_err(|e| e.to_string())?;
    assert!(current.main.is_some() && donor.is_none() && held.is_some());
    drop((current, held, job, completion.memory, rig.owner));
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
