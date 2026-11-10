// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual hidden fixture processes qualify Windows job lifetime and nested creation semantics.
use super::*;
use std::{
    ffi::OsString,
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
};
use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError, WAIT_TIMEOUT},
        System::{JobObjects::*, Threading::*},
    },
    core::{BOOL, PCWSTR, PWSTR},
};
const CHILD: &str = "NATIVE_JOB_PROBE_OUTER";
// Windows SDK JOB_OBJECT_QUERY; importing the SystemServices feature is unnecessary here.
const JOB_QUERY_ACCESS: u32 = 0x0004;
const TEST: &str =
    "application_launch::tests::containment::nested_job_creation_preserves_outer_membership";

struct Job(OwnedHandle);
impl Job {
    fn create(name: &[u16], kill: bool) -> TestResultValue<Self> {
        // SAFETY: fixed unique UTF-16 name remains live; returned handle is adopted immediately.
        let raw = unsafe { CreateJobObjectW(None, PCWSTR(name.as_ptr())) }?;
        // SAFETY: read the creating call's status before another native call can replace it.
        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        // SAFETY: CreateJobObject returned a uniquely owned real handle.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(raw.0) });
        if existed {
            return Err("Fixture Job already exists".into());
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        if kill {
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        }
        // SAFETY: exact owned fixture Job and complete initialized structure.
        unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }?;
        Ok(job)
    }
    fn open(name: &[u16]) -> windows::core::Result<Self> {
        // SAFETY: borrowed NUL-terminated name; query-only, non-inheritable handle.
        let raw = unsafe { OpenJobObjectW(JOB_QUERY_ACCESS, false, PCWSTR(name.as_ptr())) }?;
        // SAFETY: successful opener returns one uniquely owned handle.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw.0) }))
    }
    fn raw(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }
    fn active(&self) -> TestResultValue<u32> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: live query-capable handle and correctly sized initialized output.
        unsafe {
            QueryInformationJobObject(
                Some(self.raw()),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut info).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
        }?;
        Ok(info.ActiveProcesses)
    }
    fn contains(&self, process: HANDLE) -> TestResultValue<bool> {
        let mut contained = BOOL(0);
        // SAFETY: both handles remain owned for this query.
        unsafe { IsProcessInJob(process, Some(self.raw()), &mut contained) }?;
        Ok(contained.as_bool())
    }
}
type TestResultValue<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Process {
    process: OwnedHandle,
    thread: OwnedHandle,
}
impl Process {
    fn raw(&self) -> HANDLE {
        HANDLE(self.process.as_raw_handle())
    }
    fn finish(&self, resume: bool) -> TestResultValue<u32> {
        if resume {
            // SAFETY: uniquely owned primary thread was created suspended exactly once.
            if unsafe { ResumeThread(HANDLE(self.thread.as_raw_handle())) } != 1 {
                return self.abort().and(Err("Unexpected fixture suspend count".into()));
            }
        }
        // SAFETY: observation uses the retained actual process, never a PID lookup.
        if unsafe { WaitForSingleObject(self.raw(), INFINITE) } != WAIT_OBJECT_0 {
            return self.abort().and(Err("Fixture exit observation expired".into()));
        }
        let mut code = 0;
        // SAFETY: process has signaled and the output remains writable through the call.
        unsafe { GetExitCodeProcess(self.raw(), &mut code) }?;
        Ok(code)
    }
    fn abort(&self) -> TestResultValue<u32> {
        // SAFETY: exclusively this test's actual created child, never a user process or PID lookup.
        unsafe { TerminateProcess(self.raw(), 71) }?;
        // SAFETY: keep the exact process handle until actual exit has been observed.
        if unsafe { WaitForSingleObject(self.raw(), INFINITE) } != WAIT_OBJECT_0 {
            return Err("Owned fixture failed to terminate".into());
        }
        Ok(71)
    }
}
struct Attributes {
    data: Box<[usize; 128]>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.data.as_mut_ptr().cast())
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: stable aligned allocation still owns the initialized list.
            unsafe { DeleteProcThreadAttributeList(self.pointer()) };
        }
    }
}
/// Create only this test executable, with a fixed test selector or its inert --list mode.
fn spawn(job: &Job, child: Option<&str>, breakaway: bool) -> TestResultValue<Process> {
    let exe = std::env::current_exe()?;
    let args: Vec<OsString> = if child.is_some() {
        vec!["--exact".into(), TEST.into()]
    } else {
        vec!["--list".into(), "__no_fixture_tests__".into()]
    };
    let mut command = WindowsLaunchCommand::from_os(
        &exe,
        exe.parent().ok_or("Executable directory missing")?,
        &args,
    )?
    .into_native_parts();
    let mut snapshot = Vec::new();
    for (name, value) in std::env::vars_os().filter(|(name, _)| name != CHILD) {
        snapshot.extend(name.encode_wide());
        snapshot.push(b'=' as u16);
        snapshot.extend(value.encode_wide());
        snapshot.push(0);
    }
    snapshot.push(0);
    let name: Vec<_> = CHILD.encode_utf16().collect();
    let value: Vec<_> = child.unwrap_or_default().encode_utf16().collect();
    let overrides = if child.is_some() {
        vec![crate::EnvironmentOverride::Set { name: &name, value: &value }]
    } else {
        vec![]
    };
    let environment = crate::LaunchEnvironment::from_snapshot_utf16(&snapshot, &overrides)?;
    let mut list = Attributes { data: Box::new([0; 128]), initialized: false };
    let mut bytes = 0;
    // SAFETY: documented sizing query does not dereference an attribute list.
    let sized = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut bytes) };
    if let Err(cause) = &sized
        && cause.code().0 as u32 != 0x8007007a
    {
        return Err(cause.clone().into());
    }
    if sized.is_ok() || bytes == 0 || bytes > size_of::<[usize; 128]>() {
        return Err("Unexpected attribute storage size".into());
    }
    // SAFETY: stable aligned allocation covers the measured size.
    unsafe { InitializeProcThreadAttributeList(Some(list.pointer()), 1, None, &mut bytes) }?;
    list.initialized = true;
    let jobs = [job.raw()];
    // SAFETY: actual fixture Job remains owned and its value remains stable through creation.
    unsafe {
        UpdateProcThreadAttribute(
            list.pointer(),
            0,
            PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
            Some(jobs.as_ptr().cast()),
            size_of::<HANDLE>(),
            None,
            None,
        )
    }?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList = list.pointer();
    let mut process = PROCESS_INFORMATION::default();
    let mut flags = CREATE_SUSPENDED
        | CREATE_NO_WINDOW
        | CREATE_UNICODE_ENVIRONMENT
        | EXTENDED_STARTUPINFO_PRESENT;
    if breakaway {
        flags |= CREATE_BREAKAWAY_FROM_JOB;
    }
    // SAFETY: bounded validated buffers, environment and attributes outlive this call. No handles inherit.
    let created = unsafe {
        CreateProcessW(
            PCWSTR(command.application_name.as_ptr()),
            Some(PWSTR(command.command_line.as_mut_ptr())),
            None,
            None,
            false,
            flags,
            Some(environment.as_utf16().as_ptr().cast()),
            PCWSTR(command.working_directory.as_ptr()),
            &startup.StartupInfo,
            &mut process,
        )
    };
    // SAFETY: every native output is immediately adopted, even on a failed creation result.
    let process_handle = (!process.hProcess.is_invalid())
        .then(|| unsafe { OwnedHandle::from_raw_handle(process.hProcess.0) });
    // SAFETY: distinct uniquely owned primary thread output follows the same adoption rule.
    let thread = (!process.hThread.is_invalid())
        .then(|| unsafe { OwnedHandle::from_raw_handle(process.hThread.0) });
    if let Err(cause) = created {
        if let Some(process) = &process_handle {
            // SAFETY: this is exclusively the native partial output just adopted above.
            unsafe { TerminateProcess(HANDLE(process.as_raw_handle()), 71) }?;
            // SAFETY: the same actual owned partial process remains retained through observation.
            if unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), INFINITE) }
                != WAIT_OBJECT_0
            {
                return Err("Partial fixture creation did not terminate".into());
            }
        }
        return Err(cause.into());
    }
    Ok(Process {
        process: process_handle.ok_or("Native process output absent")?,
        thread: thread.ok_or("Native thread output absent")?,
    })
}
fn name() -> TestResultValue<(tempfile::TempDir, String, Vec<u16>)> {
    let root = tempfile::tempdir()?;
    let name = format!(
        "Local\\SugarSubstituteJobProbe-{}",
        root.path().file_name().ok_or("Fixture identity absent")?.to_string_lossy()
    );
    let wide = name.encode_utf16().chain([0]).collect();
    Ok((root, name, wide))
}

#[test]
fn named_job_absence_does_not_prove_process_exit() -> TestResult {
    worker::run_reserved(1024 * 1024, |context, _| {
        let _memory = context.memory().claim(262144)?;
        let (_directory, _, name) = name()?;
        let job = Job::create(&name, false)?;
        let child = spawn(&job, None, false)?;
        let initial = (|| -> TestResultValue<(u32, bool)> {
            let reopened = Job::open(&name)?;
            Ok((reopened.active()?, reopened.contains(child.raw())?))
        })();
        drop(job);
        let reopened = Job::open(&name);
        // SAFETY: the child is still suspended and its original process handle remains owned.
        let still_alive = unsafe { WaitForSingleObject(child.raw(), 0) };
        child.abort()?;
        let (active, contained) = initial?;
        assert_eq!(active, 1);
        assert!(contained);
        assert_eq!(still_alive, WAIT_TIMEOUT);
        let error = match reopened {
            Ok(_) => {
                return Err(
                    "This Windows host retained the Job namespace after final closure".into()
                );
            }
            Err(error) => error,
        };
        assert_eq!(error.code().0 as u32, 0x80070002);
        Ok(())
    })
}

#[test]
fn nested_job_creation_preserves_outer_membership() -> TestResult {
    worker::run_reserved(1024 * 1024, |context, _| {
        let _memory = context.memory().claim(262144)?;
        if let Some(input) = std::env::var_os(CHILD) {
            let (outer, receipt): (String, std::path::PathBuf) =
                serde_json::from_str(input.to_str().ok_or("Fixture input is not UTF-8")?)?;
            let outer: Vec<_> = outer.encode_utf16().chain([0]).collect();
            let outer = Job::open(&outer)?;
            // SAFETY: borrowed current-process pseudo-handle is valid for this query.
            assert!(outer.contains(unsafe { GetCurrentProcess() })?);
            for breakaway in [true, false] {
                let (_directory, _, name) = name()?;
                let job = Job::create(&name, true)?;
                match spawn(&job, None, breakaway) {
                    Err(cause) if breakaway => {
                        let native = cause
                            .downcast_ref::<windows::core::Error>()
                            .ok_or("Non-native creation rejection")?;
                        assert_eq!(native.code().0 as u32, 0x80070005);
                    }
                    result => {
                        let child = result?;
                        let observed = (outer.contains(child.raw()), job.contains(child.raw()));
                        let exit = child.finish(true)?;
                        assert!(!breakaway, "Unexpected breakaway admission");
                        assert!(observed.0? && observed.1?);
                        assert_eq!(exit, 0);
                    }
                }
            }
            let mut output =
                std::fs::OpenOptions::new().write(true).create_new(true).open(receipt)?;
            std::io::Write::write_all(&mut output, b"nested-ok")?;
        } else {
            let (directory, name, wide) = name()?;
            let receipt = directory.path().join("nested-receipt");
            let input = serde_json::to_string(&(name, &receipt))?;
            let job = Job::create(&wide, true)?;
            let child = spawn(&job, Some(&input), false)?;
            let exit = child.finish(true)?;
            drop(child);
            assert_eq!(exit, 0, "Nested job fixture failed");
            assert_eq!(std::fs::read(receipt)?, b"nested-ok");
        }
        Ok(())
    })
}

#[test]
fn query_handle_extends_kill_on_close_ownership_until_it_is_released() -> TestResult {
    worker::run_reserved(1024 * 1024, |context, _| {
        let _memory = context.memory().claim(262144)?;
        let (_directory, _, name) = name()?;
        let job = Job::create(&name, true)?;
        let observer = Job::open(&name)?;
        let child = spawn(&job, None, false)?;
        drop(job);
        // SAFETY: the suspended child's original process handle is still retained.
        let while_observed = unsafe { WaitForSingleObject(child.raw(), 0) };
        drop(observer);
        let exit = child.finish(false)?;
        assert_eq!(while_observed, WAIT_TIMEOUT);
        assert_ne!(exit, 259, "Last Job closure must terminate the owned fixture");
        Ok(())
    })
}
