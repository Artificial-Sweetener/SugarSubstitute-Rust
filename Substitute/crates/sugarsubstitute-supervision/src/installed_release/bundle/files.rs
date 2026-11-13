// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Install each selected physical cell before its existing native producer runs.
use super::super::{image, manifest::Pair, names::CapturePathRole};
use super::*;
use crate::durable_storage::protected::{
    account,
    native_file::{FileGrant, NativeFileAdmission},
    native_name::NativeName,
    native_phase::NativePhase,
    paths,
};
use std::{cell::RefCell, ffi::OsStr};

pub(super) fn phase<'a>(
    origin: &'a AllocationOrigin,
    scratch: &'a mut CaptureScratch,
    causes: &'a mut NativeCauseDestinations,
) -> NativePhase<'a> {
    NativePhase::Selected { original: origin, scratch, causes: RefCell::new(causes) }
}
/// Construct the sole native token cell before invoking the common account producer.
pub(super) fn account(fields: &mut Fields, origin: &AllocationOrigin) -> Result<(), ClaimError> {
    let cell = fields.token.as_mut().ok_or_else(missing)?;
    let mut phase = phase(origin, &mut fields.scratch, &mut cell.causes);
    let (sid, _session) = account::resolve_current_cell(
        &mut cell.handle,
        account::PrimaryAccountAccess::Query,
        &mut phase,
    )?;
    phase.check()?;
    fields.sid = Some(sid);
    Ok(())
}
/// Pin actual ancestors in the existing root-first order, without owned path copies.
pub(super) fn ancestors(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    reconcile: bool,
) -> Result<(), ClaimError> {
    let current = fields.current.as_ref().ok_or_else(missing)?.clone();
    let path = current.path().map_err(ClaimError::capture_allocation)?;
    let count = paths::ancestor_paths(path).map_err(|_| missing())?.count();
    if count > 40 {
        return Err(missing());
    }
    for (index, path) in paths::ancestor_paths(path).map_err(|_| missing())?.enumerate() {
        let storage = if reconcile { 40 + index } else { index };
        let prior = if reconcile {
            fields.directories[index].as_ref().and_then(|cell| cell.identity)
        } else {
            None
        };
        let cell = fields.directories[storage].as_mut().ok_or_else(missing)?;
        let mut phase = phase(origin, &mut fields.scratch, &mut cell.causes);
        paths::open_cell(path, true, true, &mut cell.handle, &mut phase)?;
        let identity = paths::file_identity(&cell.handle, &mut phase)?;
        cell.identity = Some(identity);
        if reconcile && prior != Some(identity) {
            return Err(phase.invalid("selected_changed_ancestry"));
        }
    }
    Ok(())
}
/// A fixed label from authenticated bytes chooses one of the eight retained slots.
pub(super) fn open(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    index: usize,
    role: CapturePathRole,
    label: Option<&str>,
    bound: u64,
) -> Result<(), ClaimError> {
    let current = fields.current.as_ref().ok_or_else(missing)?.clone();
    let name = match label {
        None => current,
        Some(label) => {
            let root = current
                .path()
                .map_err(ClaimError::capture_allocation)?
                .parent()
                .ok_or_else(missing)?;
            let destination = origin.name(role).map_err(ClaimError::capture_allocation)?;
            match destination.produce(root, Some(OsStr::new(label))) {
                Ok(name) => name,
                Err(cause) => {
                    let causes =
                        fields.causes.get_mut(1).and_then(Option::as_mut).ok_or_else(missing)?;
                    return Err(causes.selected_error(
                        "capture_name",
                        super::super::CaptureBoundedCause::Name(cause),
                        false,
                    ));
                }
            }
        }
    };
    let loan = origin
        .reserve(
            CaptureRegion::Metadata,
            NativeFile::diagnostic_owner_layout().map_err(|_| missing())?.retained
                + SharedCaptureLoan::control_bytes(),
        )
        .map_err(ClaimError::capture_allocation)?;
    let allocation = SharedCaptureLoan::new(loan).map_err(ClaimError::capture_allocation)?;
    let PreparedFile { output, causes } = fields.prepared[index].take().ok_or_else(missing)?;
    let cell = NativeFile::reserve(
        (index + 1) as u64,
        bound,
        FileGrant::ReadOnly,
        &mut Some((
            NativeName::Selected(name),
            NativeFileAdmission::Selected { original: origin.clone(), allocation, output, causes },
        )),
    )?;
    fields.files[index] = Some(cell);
    let context = origin
        .original()
        .original_context()
        .map_err(|cause| {
            fields.causes[1]
                .as_mut()
                .map_or_else(missing, |causes| causes.selected_control(cause, false))
        })?
        .clone();
    let sid = fields.sid.as_ref().ok_or_else(missing)?.bytes()?;
    let causes = fields.causes[1].as_mut().ok_or_else(missing)?;
    let mut phase = phase(origin, &mut fields.scratch, causes);
    let file = fields.files[index].as_mut().ok_or_else(missing)?;
    file.open(Some(sid), &context, &context, &mut phase)?;
    let facts = file.observe(Some(sid), &context, &mut phase)?;
    file.fields_mut()?.facts = Some(facts);
    Ok(())
}
/// Read the fully observed original ingress file into its separately retained bound.
pub(super) fn ingress(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    index: usize,
    signature: bool,
) -> Result<(), ClaimError> {
    let context = origin.original().original_context().map_err(|_| missing())?.clone();
    let causes = fields.causes[2].as_mut().ok_or_else(missing)?;
    let mut phase = phase(origin, &mut fields.scratch, causes);
    let file = fields.files[index].as_mut().ok_or_else(missing)?;
    let bytes =
        usize::try_from(file.fields()?.facts.ok_or_else(missing)?.bytes).map_err(|cause| {
            phase.error(
                "selected_ingress_length",
                crate::claim_error::BoundedNativeCause::Integer(cause),
            )
        })?;
    let bound = if signature { 384 } else { 65_536 };
    if bytes == 0 || bytes > bound || (signature && bytes != 384) {
        return Err(phase.invalid("selected_ingress_bound"));
    }
    let output = if signature { &mut fields.signature } else { &mut fields.manifest };
    output.try_reserve_exact(bound).map_err(|cause| {
        phase.error(
            "selected_ingress_reserve",
            crate::claim_error::BoundedNativeCause::Reserve(cause),
        )
    })?;
    if output.capacity() > bound {
        return Err(phase.invalid("selected_ingress_capacity"));
    }
    output.resize(bytes, 0);
    file.read(0, output, &context, &mut phase)?;
    Ok(())
}
/// Authenticate executable bytes and their signed PE identity without opening symbol files.
pub(super) fn verify_image(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    pe: usize,
    pair: &Pair<'_>,
    kind: image::PeKind,
) -> Result<(), ClaimError> {
    let causes = fields.causes[3].as_mut().ok_or_else(missing)?;
    let mut phase = phase(origin, &mut fields.scratch, causes);
    let facts =
        fields.files[pe].as_ref().ok_or_else(missing)?.fields()?.facts.ok_or_else(missing)?;
    if facts.bytes != pair.pe_length || facts.sha256 != pair.pe_sha256 {
        return Err(phase.invalid("selected_signed_file_mismatch"));
    }
    let file = fields.files[pe].as_mut().ok_or_else(missing)?;
    let mut reader = file.reader(&mut phase)?;
    let actual = match image::read_pe(&mut reader) {
        Ok(actual) => actual,
        Err(image::ImageError::Read(cause)) => return Err(cause),
        Err(image::ImageError::Invalid(reason)) => return Err(phase.invalid(reason)),
    };
    if actual.kind != kind
        || actual.image_size != pair.image_size
        || actual.timestamp != pair.timestamp
        || actual.checksum != pair.checksum
        || actual.pdb.guid != pair.pdb_guid
        || actual.pdb.age != pair.pdb_age
    {
        return Err(phase.invalid("selected_pe_association"));
    }
    Ok(())
}

/// Match authenticated executable and symbol bytes before admitting a symbol consumer.
pub(super) fn verify_pair(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    pe: usize,
    pdb: usize,
    pair: &Pair<'_>,
    kind: image::PeKind,
) -> Result<(), ClaimError> {
    verify_image(fields, origin, pe, pair, kind)?;
    let causes = fields.causes[3].as_mut().ok_or_else(missing)?;
    let mut phase = phase(origin, &mut fields.scratch, causes);
    let file = fields.files[pdb].as_mut().ok_or_else(missing)?;
    let facts = file.fields()?.facts.ok_or_else(missing)?;
    if facts.bytes != pair.pdb_length || facts.sha256 != pair.pdb_sha256 {
        return Err(phase.invalid("selected_signed_file_mismatch"));
    }
    let mut reader = file.reader(&mut phase)?;
    let actual = match image::read_pdb(&mut reader) {
        Ok(actual) => actual,
        Err(image::ImageError::Read(cause)) => return Err(cause),
        Err(image::ImageError::Invalid(reason)) => return Err(phase.invalid(reason)),
    };
    if actual.guid != pair.pdb_guid || actual.age != pair.pdb_age {
        return Err(phase.invalid("selected_pdb_association"));
    }
    Ok(())
}
