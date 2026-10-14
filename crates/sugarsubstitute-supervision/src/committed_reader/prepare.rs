// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open the same protected namespace and exclusive existing writer fence.
use super::*;
use crate::{
    installed_release::names::CapturePathRole,
    protected_storage::{
        account,
        directories::{self, Mode, RelativeDirectory},
        paths, security,
    },
};
use std::ffi::OsStr;
fn name(
    original: &AllocationOrigin,
    role: CapturePathRole,
    path: &std::path::Path,
    child: Option<&OsStr>,
    causes: &mut NativeCauseDestinations,
) -> Result<NativeName, ClaimError> {
    let destination = original.name(role).map_err(ClaimError::capture_allocation)?;
    destination.produce(path, child).map(NativeName::Selected).map_err(|cause| {
        causes.selected_error(
            "committed_reader_name",
            crate::installed_release::CaptureBoundedCause::Name(cause),
            false,
        )
    })
}
pub(super) fn prepare(fields: &mut Fields, original: &AllocationOrigin) -> Result<(), ClaimError> {
    if fields.ready {
        return Ok(());
    }
    if fields.attempted {
        return Err(fields.failure.clone().unwrap_or_else(missing));
    }
    fields.attempted = true;
    let result = prepare_once(fields, original);
    if let Err(cause) = &result {
        fields.failure = Some(cause.clone());
    }
    result
}
fn prepare_once(fields: &mut Fields, original: &AllocationOrigin) -> Result<(), ClaimError> {
    let context = fields.context(original)?;
    let mut phase = super::phase(original, &mut fields.scratch, &mut fields.token_causes);
    let (sid, session) = account::resolve_current_cell(
        &mut fields.token,
        account::PrimaryAccountAccess::Query,
        &mut phase,
    )?;
    if sid.bytes()? != fields.namespace.resolved()?.account_sid
        || session != fields.namespace.resolved()?.session_id
    {
        return Err(phase.invalid("committed_reader_account_changed"));
    }
    fields.sid = Some(sid);
    fields.root = Some(name(
        original,
        CapturePathRole::CurrentImage,
        &fields.namespace.resolved()?.directory,
        None,
        &mut fields.causes,
    )?);
    let causes = &mut fields.directory_causes[0];
    let output = fields.quota.reserve_selected(causes)?;
    fields.directories.push(Handle::reserve_capture(&mut Some((original.clone(), output)))?);
    let root = fields.root.as_ref().ok_or_else(missing)?;
    let mut phase = super::phase(original, &mut fields.scratch, causes);
    paths::open_cell(root.path()?, true, true, &mut fields.directories[0], &mut phase)?;
    let resolved = fields.namespace.resolved()?;
    if fields.namespace.handles()?.get(resolved.directory_slot).is_none_or(|pin| !pin.is_armed())
        || paths::file_identity(&fields.directories[0], &mut phase)? != resolved.directory_identity
    {
        return Err(phase.invalid("committed_reader_root_changed"));
    }
    let sid = fields.sid.as_ref().ok_or_else(missing)?.bytes()?;
    security::verify(&fields.directories[0], sid, &mut phase)?;
    let destination = phase.name_destination()?;
    if paths::final_path(&fields.directories[0], &mut phase, destination)?.path()? != root.path()? {
        return Err(phase.invalid("committed_reader_root_canonical"));
    }
    let hex = crate::protected_storage::diagnostic::namespace_component(fields.request.namespace());
    let leaf = std::str::from_utf8(&hex).map_err(|_| missing())?;
    let mut child = [0_u8; 88];
    let prefix = br"diagnostics\incident-v1\";
    let length = prefix.len() + leaf.len();
    child[..prefix.len()].copy_from_slice(prefix);
    child[prefix.len()..length].copy_from_slice(leaf.as_bytes());
    let child = std::str::from_utf8(&child[..length]).map_err(|_| missing())?;
    fields.directory = Some(name(
        original,
        CapturePathRole::Manifest,
        root.path()?,
        Some(OsStr::new(child)),
        &mut fields.causes,
    )?);
    for depth in (0..3).rev() {
        let index = fields.directories.len();
        let causes = &mut fields.directory_causes[index];
        let output = fields.quota.reserve_selected(causes)?;
        let output = Handle::reserve_capture(&mut Some((original.clone(), output)))?;
        let path = fields
            .directory
            .as_ref()
            .ok_or_else(missing)?
            .path()?
            .ancestors()
            .nth(depth)
            .ok_or_else(missing)?;
        let mut phase = super::phase(original, &mut fields.scratch, causes);
        let result = directories::open_relative(
            &mut fields.directories,
            sid,
            crate::native_claims::files::Budget::SelectedRelease(original.clone()),
            RelativeDirectory {
                path,
                parent: index - 1,
                mode: Mode::Existing,
                output: Some(output),
            },
            &context,
            &mut phase,
        );
        phase.post_context(result, &context)?;
    }
    let path = name(
        original,
        CapturePathRole::Signature,
        fields.directory.as_ref().ok_or_else(missing)?.path()?,
        Some(OsStr::new("writer.lock")),
        &mut fields.causes,
    )?;
    let PreparedFile { output, causes } = fields.writer_output.take().ok_or_else(missing)?;
    let handle = Handle::reserve_capture(&mut Some((original.clone(), output)))?;
    let mut phase = super::phase(original, &mut fields.scratch, &mut fields.causes);
    HeldFile::acquire_existing_into(
        &mut fields.writer,
        path,
        fields.sid.as_ref().ok_or_else(missing)?.clone(),
        handle,
        causes,
        &mut phase,
    )?;
    phase.check()?;
    fields.ready = true;
    Ok(())
}
pub(super) fn object_name(
    fields: &mut Fields,
    original: &AllocationOrigin,
    object: CommittedReadObject,
) -> Result<NativeName, ClaimError> {
    let leaf = crate::protected_storage::record_file::DiagnosticLeaf::object(
        object.object(),
        fields.request.namespace(),
    )
    .map_err(|_| missing())?;
    let role = match object.index() {
        0 => CapturePathRole::CurrentPdb,
        1 => CapturePathRole::ClientImage,
        _ => CapturePathRole::ClientPdb,
    };
    name(
        original,
        role,
        fields.directory.as_ref().ok_or_else(missing)?.path()?,
        Some(OsStr::new(leaf.text().map_err(|_| missing())?)),
        &mut fields.causes,
    )
}
