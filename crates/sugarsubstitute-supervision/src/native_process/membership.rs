// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a complete bounded Job membership snapshot with verified kernel identities.
use super::*;
use windows::Win32::{
    Foundation::FILETIME,
    System::{
        JobObjects::{IsProcessInJob, JobObjectBasicProcessIdList, QueryInformationJobObject},
        Threading::{
            GetCurrentProcess, GetProcessId, GetProcessTimes, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        },
    },
};

/// Query current-process matching facts without enrolling claims or creating a family.
/// The borrowed pseudo-handle is never exposed, closed or used for process control.
pub(crate) fn current_identity(
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<Identity, RegistryError> {
    crate::durable_storage::check(context)
        .map_err(|cause| RegistryError::new("invocation_process_control", cause))?;
    // SAFETY: The OS current-process pseudo-handle is borrowed for synchronous queries only.
    let process = unsafe { GetCurrentProcess() };
    identity_checked(process, &mut || {
        crate::durable_storage::check(context)
            .map_err(|cause| RegistryError::new("invocation_process_control", cause))
    })
}

pub(super) fn identity(process: HANDLE) -> Result<Identity, RegistryError> {
    identity_checked(process, &mut || Ok(()))
}
/// Query one retained process with a separate original check before each effect.
pub(super) fn identity_checked(
    process: HANDLE,
    check: &mut dyn FnMut() -> Result<(), RegistryError>,
) -> Result<Identity, RegistryError> {
    check()?;
    // SAFETY: caller retains this process handle for the complete query sequence.
    let pid = unsafe { GetProcessId(process) };
    if pid == 0 {
        return Err(RegistryError::new("process_id", std::io::Error::last_os_error()));
    }
    let (mut birth, mut exit, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    check()?;
    // SAFETY: all outputs are initialized stack values; process ownership remains live.
    unsafe { GetProcessTimes(process, &mut birth, &mut exit, &mut kernel, &mut user) }
        .map_err(|cause| RegistryError::new("process_birth", cause))?;
    let birth = u64::from(birth.dwHighDateTime) << 32 | u64::from(birth.dwLowDateTime);
    if birth == 0 {
        return Err(RegistryError::invalid("process_birth"));
    }
    Ok(Identity { pid, birth })
}
/// Require original control before exact retained Job membership observation.
pub(super) fn verify_job_checked(
    process: HANDLE,
    job: HANDLE,
    check: &mut dyn FnMut() -> Result<(), RegistryError>,
) -> Result<(), RegistryError> {
    check()?;
    let mut member = windows::core::BOOL::default();
    // SAFETY: exact retained process and Job handles, initialized output.
    unsafe { IsProcessInJob(process, Some(job), &mut member) }
        .map_err(|cause| RegistryError::new("process_job_membership", cause))?;
    if !member.as_bool() {
        return Err(RegistryError::invalid("process_job_membership"));
    }
    Ok(())
}
impl WindowsPort {
    pub(super) fn inventory_native(&self, family: &mut Family) -> Result<Inventory, RegistryError> {
        let job = handle(family.job, "inventory_job")?;
        family.query.fill(0);
        let capacity = family.query.len() * std::mem::size_of::<usize>();
        let mut returned = 0;
        family.check_native()?;
        // SAFETY: aligned fixed storage covers the two u32 counts plus all256 ULONG_PTR identities.
        let result = unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectBasicProcessIdList,
                family.query.as_mut_ptr().cast(),
                capacity as u32,
                Some(&mut returned),
            )
        };
        if let Err(cause) = result {
            family.incomplete = true;
            return Err(RegistryError::new("complete_job_inventory", cause));
        }
        // SAFETY: successful native write covers this fixed header; aligned storage is initialized.
        let assigned = unsafe { family.query.as_ptr().cast::<u32>().read() } as usize;
        // SAFETY: second count is within the same initialized native header.
        let listed = unsafe { family.query.as_ptr().cast::<u32>().add(1).read() } as usize;
        if !complete_shape(assigned, listed, returned as usize, capacity) {
            family.incomplete = true;
            return Err(RegistryError::invalid("complete_job_inventory"));
        }
        for offset in 0..listed {
            // SAFETY: successful complete count/length validation establishes each ULONG_PTR entry.
            let raw_pid = unsafe {
                family.query.as_ptr().cast::<u8>().add(8).cast::<usize>().add(offset).read()
            };
            let pid =
                u32::try_from(raw_pid).map_err(|_| RegistryError::invalid("inventory_pid"))?;
            if pid == 0 {
                family.incomplete = true;
                return Err(RegistryError::invalid("inventory_pid"));
            }
            if let Some(member) =
                family.members.iter().flatten().find(|member| member.identity.pid == pid)
            {
                let process = handle(member.handle, "retained_member")?;
                if identity_checked(process, &mut || family.check_native())? != member.identity {
                    family.incomplete = true;
                    return Err(RegistryError::invalid("retained_member_birth"));
                }
                verify_job_checked(process, job, &mut || family.check_native())?;
                continue;
            }
            if family.count == MAX_MEMBERS {
                family.incomplete = true;
                return Err(RegistryError::invalid("retained_inventory_overflow"));
            }
            family.check_native()?;
            // SAFETY: PID is only a query result; opening alone conveys no authority.
            // The returned handle is adopted before birth/membership checks and never used for termination.
            let process = unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid)
            }
            .map_err(|cause| {
                family.incomplete = true;
                RegistryError::new("open_inventory_member", cause)
            })?;
            let index = family.count;
            family.members[index] = Some(Member {
                handle: Some(process.0 as usize),
                identity: Identity { pid, birth: 0 },
                exited: false,
            });
            family.count += 1;
            let verified = identity_checked(process, &mut || family.check_native())?;
            verify_job_checked(process, job, &mut || family.check_native())?;
            if let Some(member) = &mut family.members[index] {
                member.identity = verified;
            }
        }
        Ok(Inventory { empty: listed == 0, count: family.count })
    }
}
fn complete_shape(assigned: usize, listed: usize, returned: usize, capacity: usize) -> bool {
    assigned == listed
        && listed <= MAX_MEMBERS
        && returned <= capacity
        && returned >= 8 + listed * std::mem::size_of::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_fixed_inventory_accepts256_and_rejects_overflow_partial_or_short_results() {
        let full = 8 + MAX_MEMBERS * std::mem::size_of::<usize>();
        assert!(complete_shape(256, 256, full, full));
        assert!(complete_shape(0, 0, 8, full));
        assert!(!complete_shape(257, 257, full, full));
        assert!(!complete_shape(257, 256, full, full));
        assert!(!complete_shape(256, 256, full - 1, full));
        assert!(!complete_shape(0, 0, 7, full));
        assert!(!complete_shape(0, 0, full + 1, full));
        assert!(!complete_shape(usize::MAX, usize::MAX, full, full));
    }
}
