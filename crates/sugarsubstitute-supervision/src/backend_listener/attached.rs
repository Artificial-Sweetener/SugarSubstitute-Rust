// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain read-only attached peer identity without acquiring any stop authority.
use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::MemoryClaim;
struct AttachedState {
    handle: usize,
    opened_pid: u32,
    retired: bool,
    identity: Option<BackendPeerIdentity>,
    claim: Option<MemoryClaim>,
}
impl Drop for AttachedState {
    fn drop(&mut self) {
        if self.handle != 0 {
            if let Some(claim) = self.claim.take() {
                std::mem::forget(claim);
            }
            tracing::error!(
                operation = "attached_listener_owner_loss",
                "Read-only process identity handle remains retained"
            );
        }
    }
}
/// Own the first actual connected attached peer's read-only PID/birth handle.
/// Construction is inert; this lease cannot stop, launch or enroll the attached process.
pub struct AttachedListenerLease {
    retained: Arc<Mutex<AttachedState>>,
    open: AtomicBool,
}
impl Default for AttachedListenerLease {
    fn default() -> Self {
        Self::new()
    }
}
impl AttachedListenerLease {
    /// Create inert retained custody before submitting any HTTP/native job.
    pub fn new() -> Self {
        Self {
            retained: Arc::new(Mutex::new(AttachedState {
                handle: 0,
                opened_pid: 0,
                retired: false,
                identity: None,
                claim: None,
            })),
            open: AtomicBool::new(true),
        }
    }
    /// Observe the explicit closure fence without locking or making native calls.
    pub fn is_live(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
    /// Pin and qualify the exact current server-side established connection owner.
    /// # Errors
    /// Rejects PID reuse, process exit, ambiguous tuples, changes or expired work.
    pub fn qualify_connection(
        &self,
        connection: ConnectedLoopback,
        storage: &mut TcpOwnerWorkspace,
        context: &JobContext,
    ) -> Result<BackendPeerIdentity, RegistryError> {
        if !self.is_live() {
            return Err(RegistryError::invalid("attached_listener_retiring"));
        }
        let pid = table::connected_owner(connection, storage, context)?;
        let mut retained = crate::registry::lock(&self.retained);
        native::qualify(&mut retained, pid, context)?;
        if table::connected_owner(connection, storage, context)? != pid {
            return Err(RegistryError::invalid("attached_listener_changed"));
        }
        retained.identity.ok_or_else(|| RegistryError::invalid("attached_listener_identity"))
    }
    /// Close only this lease's read-only process handle on admitted finite work.
    /// # Errors
    /// Retains the original handle and charge if closure fails.
    pub fn close(&self, context: &JobContext) -> Result<(), RegistryError> {
        self.open.store(false, Ordering::Release);
        native::close(&mut crate::registry::lock(&self.retained), context)
    }
}
#[allow(unsafe_code)]
mod native {
    use super::*;
    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_FAILED, WAIT_TIMEOUT},
        System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            WaitForSingleObject,
        },
    };
    fn check(context: &JobContext) -> Result<(), RegistryError> {
        super::super::table::check(context)
    }
    pub(super) fn qualify(
        state: &mut AttachedState,
        pid: u32,
        context: &JobContext,
    ) -> Result<(), RegistryError> {
        check(context)?;
        if state.retired {
            return Err(RegistryError::invalid("attached_listener_retired"));
        }
        if state.handle == 0 {
            state.claim = Some(
                context
                    .memory()
                    .claim(64 * 1024)
                    .map_err(|e| RegistryError::new("attached_listener_memory", e))?,
            );
            // SAFETY: Explicit read-only query/synchronize access; inheritance and stop privileges are absent.
            let handle = unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid)
            }
            .map_err(|e| RegistryError::new("attached_listener_open", e))?;
            state.handle = handle.0 as usize;
            state.opened_pid = pid;
        } else if state.opened_pid != pid {
            return Err(RegistryError::invalid("attached_listener_pid_changed"));
        }
        check(context)?;
        let mut birth = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: The retained read-only process handle and all output structures live through the query.
        unsafe {
            GetProcessTimes(
                HANDLE(state.handle as *mut _),
                &mut birth,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        }
        .map_err(|e| RegistryError::new("attached_listener_birth", e))?;
        check(context)?;
        let identity = BackendPeerIdentity {
            process_id: pid,
            creation_time_100ns: (u64::from(birth.dwHighDateTime) << 32)
                | u64::from(birth.dwLowDateTime),
        };
        if state.identity.is_some_and(|previous| previous != identity) {
            return Err(RegistryError::invalid("attached_listener_birth_changed"));
        }
        // SAFETY: A zero-time read-only wait observes current liveness and does not suspend or stop the peer.
        let status = unsafe { WaitForSingleObject(HANDLE(state.handle as *mut _), 0) };
        if status == WAIT_FAILED {
            return Err(RegistryError::new(
                "attached_listener_wait",
                std::io::Error::last_os_error(),
            ));
        }
        if status != WAIT_TIMEOUT {
            return Err(RegistryError::invalid("attached_listener_exited_or_wait_failed"));
        }
        check(context)?;
        state.identity = Some(identity);
        Ok(())
    }
    pub(super) fn close(
        state: &mut AttachedState,
        context: &JobContext,
    ) -> Result<(), RegistryError> {
        check(context)?;
        if state.handle != 0 {
            // SAFETY: This slot uniquely owns only the read-only process handle, cleared after real closure.
            unsafe { CloseHandle(HANDLE(state.handle as *mut _)) }
                .map_err(|e| RegistryError::new("attached_listener_close", e))?;
            state.handle = 0;
        }
        state.identity = None;
        state.retired = true;
        state.claim = None;
        Ok(())
    }
}
