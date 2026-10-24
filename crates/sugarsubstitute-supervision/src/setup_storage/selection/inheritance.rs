// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry a selection read pin across native process creation without borrowing the run lock.
use super::{ReservedApplication, check};
use crate::setup_storage::disk_inheritance::{capture_disk, duplicate_inheritable};
use std::{
    ffi::OsString,
    fs::File,
    io,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle},
};
use sugarsubstitute_contracts::execution::JobContext;

use sugarsubstitute_contracts::application_launch::INSTALLED_LAUNCH_ARGUMENT as PREFIX;

/// Parent-owned preparation for an explicit native HANDLE_LIST containing this one file.
/// Keep this owner until process creation returns. The duplicate preserves the same Windows
/// sharing rights in the child even if the parent exits before coordinator admission.
/// A trial forwarding snapshot permits health publication and grants only an independent
/// invocation; the original run transfer remains necessary for trial admission.
/// It grants neither application-run authority nor permission to accept a trial.
pub struct LaunchInheritance {
    application: ReservedApplication,
    selection: File,
}
impl LaunchInheritance {
    /// Retain the authenticated payload, inventory and ancestry across a native launch broker.
    /// Selection ownership travels separately as the installed-entry inheritance handle.
    pub(crate) fn launch_pins(&self) -> impl Iterator<Item = &File> {
        let application = &self.application.application;
        std::iter::once(&application.inventory)
            .chain(&application.directory._pins)
            .chain(application.payload.launch_pins())
    }

    /// Verified payload retained through the actual process-creation call.
    pub fn application(&self) -> &ReservedApplication {
        &self.application
    }

    /// The sole selection handle to include in the native process inheritance allowlist.
    pub fn selection_handle(&self) -> BorrowedHandle<'_> {
        self.selection.as_handle()
    }

    /// Versioned child arguments, valid only with the actual inherited selection handle.
    pub fn arguments(&self) -> [OsString; 2] {
        [PREFIX.into(), (self.selection.as_raw_handle() as usize).to_string().into()]
    }
}
impl ReservedApplication {
    /// Prepare an inheritable duplicate while retaining verified payload and selection custody.
    /// Execute on the admitted native worker; include only the returned handle in HANDLE_LIST.
    /// # Errors
    /// Preserves cancellation and native duplication errors without releasing the caller's
    /// application-run owner or changing installation state.
    pub fn prepare_inheritance(self, context: &JobContext) -> io::Result<LaunchInheritance> {
        check(context)?;
        self.application.require_installed_launch()?;
        let selection = duplicate_inheritable(&self._selection)?;
        check(context)?;
        Ok(LaunchInheritance { application: self, selection })
    }
}

/// Child-owned inherited selection pin, captured before application resources are created.
/// Its file is not trusted until matched against the protected installation's current record.
pub struct InheritedSelection {
    pub(super) file: File,
}

/// Capture only the versioned installed-launch entry mode; other entry modes remain untouched.
/// No paths are opened, jobs started or payloads trusted at this boundary. Standard handles,
/// non-inheritable handles and non-disk objects are rejected without adopting them.
/// # Safety
/// The supplied handle must have no other Rust/native owner in this process, and the caller
/// must prevent concurrent closure/reuse until this call returns. In production, call exactly
/// once at the executable's initial entry before local resources or workers exist. Never call
/// this on arbitrary arguments after local handle-owning code has run.
/// # Errors
/// Rejects malformed/unsupported transfer arguments, invalid handles and inheritance narrowing.
pub unsafe fn capture_launch_selection(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<InheritedSelection>> {
    let Some(raw) = parse(arguments)? else { return Ok(None) };
    // SAFETY: the initial-entry caller supplies the same unique ownership contract.
    let file = unsafe { capture_disk(raw) }?;
    Ok(Some(InheritedSelection { file }))
}

fn parse(arguments: impl IntoIterator<Item = OsString>) -> io::Result<Option<usize>> {
    let mut arguments = arguments.into_iter();
    let Some(prefix) = arguments.next() else { return Ok(None) };
    if prefix != PREFIX {
        if prefix.to_str().is_some_and(|value| value.starts_with("--installed-launch-")) {
            return Err(super::super::invalid("Unsupported installed launch protocol"));
        }
        return Ok(None);
    }
    let spelling = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| super::super::invalid("Launch selection handle is absent"))?;
    let raw = spelling
        .parse::<usize>()
        .map_err(|_| super::super::invalid("Launch selection handle is invalid"))?;
    if raw == 0 || raw == usize::MAX || raw.to_string() != spelling || arguments.next().is_some() {
        return Err(super::super::invalid("Installed launch arguments are invalid"));
    }
    Ok(Some(raw))
}

#[cfg(test)]
mod tests;
