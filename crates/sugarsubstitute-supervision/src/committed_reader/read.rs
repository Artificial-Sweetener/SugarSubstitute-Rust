// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read only original immutable inode slots under the retained writer fence.
use super::*;
use crate::{
    diagnostic_storage::{DIAGNOSTIC_IO_BYTES, DiagnosticReadFacts},
    installed_release::allocation::CaptureRegion,
    protected_storage::native_file::{FileGrant, NativeFileAdmission},
};
impl CommittedDiagnosticReader {
    /// Open a closed committed object once and retain its actual pin until cleanup.
    /// # Errors
    /// Changed slots, foreign namespaces, missing files and partial native outputs fail closed.
    pub fn open(&mut self, object: CommittedReadObject) -> Result<CommittedReadSlot, ClaimError> {
        let original = self.original.clone();
        let (fields, origin) = self.parts()?;
        let result = open(fields, origin, object);
        if let Err(cause) = &result {
            fields.failure = Some(cause.clone());
        }
        result.map(|()| CommittedReadSlot { object, original })
    }
    fn validate_slot(&self, slot: &CommittedReadSlot) -> Result<usize, ClaimError> {
        let index = slot.object.index();
        if !self.original.same_origin(&slot.original)
            || self.fields.as_ref().is_none_or(|fields| fields.objects[index] != Some(slot.object))
        {
            return Err(missing());
        }
        Ok(index)
    }
    /// Return cached actual same-inode size/hash/identity; facts grant no native permission.
    /// # Errors
    /// Rejects foreign, unopened or unobserved slots.
    pub fn facts(&self, slot: &CommittedReadSlot) -> Result<DiagnosticReadFacts, ClaimError> {
        let index = self.validate_slot(slot)?;
        self.fields
            .as_ref()
            .and_then(|fields| fields.files[index].as_ref())
            .ok_or_else(missing)?
            .fields()?
            .facts
            .ok_or_else(missing)
    }
    /// Read at most64KiB from the exact copied inode under its original Core controls.
    /// # Errors
    /// Range, cancellation, deadline, short-read and changed identity failures retain every pin.
    pub fn read_chunk(
        &mut self,
        slot: &CommittedReadSlot,
        offset: u64,
        output: &mut [u8],
    ) -> Result<usize, ClaimError> {
        let index = self.validate_slot(slot)?;
        let (fields, origin) = self.parts()?;
        if let Some(cause) = &fields.failure {
            return Err(cause.clone());
        }
        let mut phase = super::phase(origin, &mut fields.scratch, &mut fields.causes);
        if output.len() > DIAGNOSTIC_IO_BYTES {
            return Err(phase.invalid("committed_reader_chunk_bound"));
        }
        let file = fields.files[index].as_mut().ok_or_else(missing)?;
        let mut reader = file.reader(&mut phase)?;
        if offset > reader.length() {
            return Err(missing());
        }
        let count =
            output.len().min((reader.length() - offset).min(DIAGNOSTIC_IO_BYTES as u64) as usize);
        let result = reader.read_exact(offset, &mut output[..count]);
        if let Err(cause) = &result {
            fields.failure = Some(cause.clone());
        }
        result.map(|()| count)
    }
    /// Revalidate the original Summary while Main is Running and its fence remains held.
    /// # Errors
    /// Rejects any changed actual inode/size/hash; cleanup never performs this read.
    pub fn check_current(&mut self, slot: &CommittedReadSlot) -> Result<(), ClaimError> {
        let index = self.validate_slot(slot)?;
        if !matches!(slot.object, CommittedReadObject::Summary(_)) {
            return Err(missing());
        }
        let (fields, origin) = self.parts()?;
        if let Some(cause) = &fields.failure {
            return Err(cause.clone());
        }
        let context = fields.context(origin)?;
        let mut phase = super::phase(origin, &mut fields.scratch, &mut fields.causes);
        let file = fields.files[index].as_mut().ok_or_else(missing)?;
        let before = file.fields()?.facts.ok_or_else(missing)?;
        let result = file
            .observe(Some(fields.sid.as_ref().ok_or_else(missing)?.bytes()?), &context, &mut phase)
            .and_then(|actual| {
                if actual == before {
                    Ok(())
                } else {
                    Err(phase.invalid("committed_reader_summary_changed"))
                }
            });
        if let Err(cause) = &result {
            fields.failure = Some(cause.clone());
        }
        result
    }
}
fn open(
    fields: &mut Fields,
    origin: &AllocationOrigin,
    object: CommittedReadObject,
) -> Result<(), ClaimError> {
    if let Some(cause) = &fields.failure {
        return Err(cause.clone());
    }
    let index = object.index();
    if object.object().namespace().is_some_and(|namespace| namespace != fields.request.namespace())
    {
        return Err(missing());
    }
    if let Some(previous) = fields.objects[index] {
        return if previous == object
            && fields.files[index]
                .as_ref()
                .is_some_and(|file| file.fields().is_ok_and(|fields| fields.facts.is_some()))
        {
            Ok(())
        } else {
            Err(missing())
        };
    }
    super::prepare::prepare(fields, origin)?;
    // Reserve the semantic slot before any producer can return a partial output.
    fields.objects[index] = Some(object);
    let name = super::prepare::object_name(fields, origin, object)?;
    let bytes = NativeFile::diagnostic_owner_layout().map_err(|_| missing())?.retained
        + SharedCaptureLoan::control_bytes();
    let allocation = SharedCaptureLoan::new(
        origin.reserve(CaptureRegion::Metadata, bytes).map_err(ClaimError::capture_allocation)?,
    )
    .map_err(ClaimError::capture_allocation)?;
    let PreparedFile { output, causes } = fields.prepared[index].take().ok_or_else(missing)?;
    let mut input = Some((
        name,
        NativeFileAdmission::Selected { original: origin.clone(), allocation, output, causes },
    ));
    let file = NativeFile::reserve(
        (index + 1) as u64,
        object.object().bound(),
        FileGrant::ReadOnly,
        &mut input,
    )?;
    fields.files[index] = Some(file);
    let context = fields.context(origin)?;
    let mut phase = super::phase(origin, &mut fields.scratch, &mut fields.causes);
    let file = fields.files[index].as_mut().ok_or_else(missing)?;
    let sid = Some(fields.sid.as_ref().ok_or_else(missing)?.bytes()?);
    file.open(sid, &context, &context, &mut phase)?;
    let facts = file.observe(sid, &context, &mut phase)?;
    file.fields_mut()?.facts = Some(facts);
    Ok(())
}
