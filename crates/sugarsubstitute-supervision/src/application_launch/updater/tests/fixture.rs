// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticated inert images and exact offscreen process observations for activation tests.
use super::*;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::Win32::{
    Foundation::{
        DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS,
        SetHandleInformation, WAIT_OBJECT_0,
    },
    System::Threading::{GetCurrentProcess, INFINITE, TerminateProcess, WaitForSingleObject},
};
pub(super) type Value<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Keep temporary directories alive until all native custody has been retired.
pub(super) fn with_image(
    body: impl FnOnce(ActivationLaunch, &std::path::Path, &JobContext) -> TestResult + Send + 'static,
) -> TestResult {
    run(move |context| {
        let image = Image::prepare(context)?;
        body(image.launch, &image.path, context)
    })
}

/// Retain authenticated fixture directories across independent execution scopes.
pub(super) struct Image {
    pub(super) launch: ActivationLaunch,
    pub(super) path: std::path::PathBuf,
    _root: tempfile::TempDir,
    _workspace: tempfile::TempDir,
}
impl Image {
    /// Prepare the same real native image custody used by single-phase containment tests.
    pub(super) fn prepare(context: &JobContext) -> Value<Self> {
        let root = worker::installation(193, context)?;
        let workspace = tempfile::tempdir()?;
        let (bytes, _) = archive(CURRENT_UPDATER_CONTRACT)?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let prepared = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?
        .stage(|| NOW, context)?;
        accept_prepared(prepared, context)?;
        let trust = TrustStore::open(root.path(), [92; 32], context)?;
        let image = trust.pin_updater(Channel::Release, 2, context)?.ok_or("Engine absent")?;
        let path = image.executable();
        let launch = ActivationLaunch::new(Box::new(image));
        launch.prepare(context)?;
        Ok(Self { launch, path, _root: root, _workspace: workspace })
    }
}

/// Replace only the executable at the external creation boundary; retain production containment.
pub(super) fn create(
    _: &mut WindowsLaunchBuffers,
    job: HANDLE,
    channel: usize,
    output: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
    create_arguments(
        &["--list".into(), "__sugar_activation_no_tests__".into()],
        job,
        channel,
        output,
    )
}

/// A real invalid invocation exits unsuccessfully without invoking a test or application role.
pub(super) fn failing(
    _: &mut WindowsLaunchBuffers,
    job: HANDLE,
    channel: usize,
    output: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
    create_arguments(
        &["--sugar-deliberately-invalid-fixture-argument".into()],
        job,
        channel,
        output,
    )
}

/// Execute only this test binary with explicit fixture arguments through production creation.
fn create_arguments(
    arguments: &[std::ffi::OsString],
    job: HANDLE,
    channel: usize,
    output: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
    let executable =
        std::env::current_exe().map_err(|cause| ClaimError::new("fixture_executable", cause))?;
    let mut command = WindowsLaunchCommand::from_os(
        &executable,
        executable.parent().ok_or_else(|| ClaimError::invalid("fixture_directory"))?,
        arguments,
    )
    .map_err(|cause| ClaimError::new("fixture_command", cause))?
    .into_native_parts();
    super::super::super::creation::contained(&mut command, job, channel, output)
}

/// A separate observation handle cannot keep a kill-on-close Job alive.
pub(super) struct Process {
    process: OwnedHandle,
    _thread: Option<OwnedHandle>,
}
impl Process {
    /// Copy an actual retained kernel process identity, never reopen by PID.
    pub(super) fn observe(launch: &ActivationLaunch) -> Value<Self> {
        let state = crate::native_claims::lock(&launch.0);
        let source = state.process.as_ref().ok_or("Process absent")?.raw();
        let mut output = HANDLE::default();
        // SAFETY: source remains owned under the lock; current-process pseudo handles are valid.
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                source,
                GetCurrentProcess(),
                &mut output,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        }?;
        // SAFETY: DuplicateHandle returned one unique owned output.
        Ok(Self { process: unsafe { OwnedHandle::from_raw_handle(output.0) }, _thread: None })
    }
    /// Add a suspended fixture member to the same actual Job without racing root execution.
    pub(super) fn member(launch: &ActivationLaunch) -> Value<Self> {
        let state = crate::native_claims::lock(&launch.0);
        let job = state.job.as_ref().ok_or("Job absent")?.raw();
        let mut command = WindowsLaunchCommand::from_os(
            &std::env::current_exe()?,
            &std::env::current_dir()?,
            &[],
        )?
        .into_native_parts();
        let mut output = PROCESS_INFORMATION::default();
        let created = create(&mut command, job, 0, &mut output);
        // SAFETY: each non-null native output is uniquely owned even on failed creation.
        let process = (!output.hProcess.is_invalid())
            .then(|| unsafe { OwnedHandle::from_raw_handle(output.hProcess.0) });
        // SAFETY: this independent primary thread output has the same unique ownership.
        let thread = (!output.hThread.is_invalid())
            .then(|| unsafe { OwnedHandle::from_raw_handle(output.hThread.0) });
        let owned = Self { process: process.ok_or("Fixture process absent")?, _thread: thread };
        created?;
        Ok(owned)
    }
    /// Borrow this fixture's actual process handle while its owning wrapper remains alive.
    fn raw(&self) -> HANDLE {
        HANDLE(self.process.as_raw_handle())
    }
    /// Distinguish a live kernel process from a retained handle to an exited process.
    pub(super) fn running(&self) -> Value<bool> {
        Ok(native::exit(self.raw())?.is_none())
    }
    /// Wait only on this fixture's actual process, for its exit event rather than elapsed time.
    pub(super) fn wait(&self) -> Value<u32> {
        // SAFETY: the owned actual process outlives the observation.
        if unsafe { WaitForSingleObject(self.raw(), INFINITE) } != WAIT_OBJECT_0 {
            return Err("Fixture process failed to settle".into());
        }
        native::exit(self.raw())?.ok_or_else(|| "Signaled fixture has no exit code".into())
    }
    /// Terminate only this fixture and require its actual terminal status before returning.
    pub(super) fn terminate(&self, code: u32) -> TestResult {
        // SAFETY: only this test's actual created process is targeted.
        unsafe { TerminateProcess(self.raw(), code) }?;
        assert_eq!(self.wait()?, code);
        Ok(())
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let result = (|| -> TestResult {
            if self.running()? {
                self.terminate(71)?;
            }
            Ok(())
        })();
        if let Err(cause) = result {
            tracing::error!(operation="activation_fixture_cleanup", %cause, "Owned fixture cleanup failed");
        }
    }
}

/// Clear close protection even during test failure before the activation owner is released.
pub(super) struct Protected(ActivationLaunch);
impl Protected {
    /// Force checked native handle retirement to fail without replacing the real Windows call.
    pub(super) fn new(launch: &ActivationLaunch) -> Value<Self> {
        set_protection(launch, true)?;
        Ok(Self(launch.clone()))
    }
    /// Restore normal handle retirement before observing the retained transfer decision.
    pub(super) fn clear(self) -> TestResult {
        set_protection(&self.0, false)
    }
}
impl Drop for Protected {
    fn drop(&mut self) {
        if let Err(cause) = set_protection(&self.0, false) {
            tracing::error!(operation="activation_fixture_unprotect", %cause, "Fixture protection cleanup failed");
        }
    }
}
/// Change close protection only on the fixture's retained process output under its owner lock.
fn set_protection(launch: &ActivationLaunch, protect: bool) -> TestResult {
    let state = crate::native_claims::lock(&launch.0);
    if let Some(process) = &state.process {
        // SAFETY: the exact owner retains this handle throughout the change under its state lock.
        unsafe {
            SetHandleInformation(
                process.raw(),
                HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                if protect { HANDLE_FLAG_PROTECT_FROM_CLOSE } else { HANDLE_FLAGS(0) },
            )
        }?;
    }
    Ok(())
}
