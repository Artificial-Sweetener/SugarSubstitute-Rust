// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded receiving paths, pinned ancestors and checked export file retirement.
mod files;
mod lifecycle;
mod names;
pub(crate) mod retention;
use crate::report_export::{ReportExportCause as E, ReportExportCode as C, ReportExportIntent};
use sugarsubstitute_execution::{ReportExportOrigin, ReportExportOutput, ReportExportRetirement};
use windows::Win32::Foundation::HANDLE;
/// Couple one real kernel handle to its pre-reserved original output guard.
pub(crate) struct Slot {
    handle: Option<HANDLE>,
    output: Option<ReportExportOutput>,
}
impl Slot {
    /// Reserve an inert slot before an OS effect can produce an output.
    fn empty() -> Self {
        Self { handle: None, output: None }
    }
    /// Borrow only this retained physical output; no path-based reopen occurs.
    fn raw(&self) -> Result<HANDLE, E> {
        self.handle.ok_or(E::Rejected(C::Identity))
    }
}
/// Retain bounded paths, ancestor pins and accepted native phase observations.
pub(crate) struct Fields {
    original: ReportExportOrigin,
    parents: [Slot; 64],
    parent_count: usize,
    stage: Slot,
    target: Slot,
    target_before: Option<files::FileFacts>,
    stage_before: Option<files::FileFacts>,
    written_bytes: u64,
    target_path: [u16; 1200],
    stage_path: [u16; 1200],
    backup_path: [u16; 1200],
    target_length: usize,
    root_length: usize,
    target_moved: bool,
    stage_activated: bool,
    stage_deleted: bool,
    target_deleted: bool,
    opened: bool,
}
/// No public constructor accepts a bare JobContext, filesystem path or native handle.
pub(crate) struct Destination {
    fields: Option<Box<Fields>>,
}
impl Destination {
    /// Allocate the fixed destination envelope from genuine original admission.
    pub(crate) fn new(original: ReportExportOrigin) -> Self {
        Self {
            fields: Some(Box::new(Fields {
                original,
                parents: std::array::from_fn(|_| Slot::empty()),
                parent_count: 0,
                stage: Slot::empty(),
                target: Slot::empty(),
                target_before: None,
                stage_before: None,
                written_bytes: 0,
                target_path: [0; 1200],
                stage_path: [0; 1200],
                backup_path: [0; 1200],
                target_length: 0,
                root_length: 0,
                target_moved: false,
                stage_activated: false,
                stage_deleted: false,
                target_deleted: false,
                opened: false,
            })),
        }
    }
    /// Borrow genuine original custody for separately activated paired cleanup.
    pub(crate) fn origin(&self) -> Result<&ReportExportOrigin, E> {
        Ok(&self.fields.as_ref().ok_or(E::Rejected(C::Identity))?.original)
    }
    /// Pin selected ancestors and predecessor before creating the sole new stage.
    pub(crate) fn open(&mut self, intent: &ReportExportIntent) -> Result<(), E> {
        let fields = self.fields.as_mut().ok_or(E::Rejected(C::Identity))?;
        if fields.opened {
            return Err(E::Rejected(C::Identity));
        }
        fields.opened = true;
        fields.original.check_effect().map_err(E::Control)?;
        names::prepare(fields, intent)?;
        files::open(fields, intent)
    }
    /// Write one bounded chunk under the original running action controls.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), E> {
        files::write(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?, bytes)
    }
    /// Observe actual synchronous flush of the same retained receiving stage.
    pub(crate) fn flush(&mut self) -> Result<(), E> {
        files::flush(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?)
    }
    /// Preserve a held predecessor, then activate the stage with no replacement race.
    pub(crate) fn activate(&mut self) -> Result<(), E> {
        lifecycle::activate(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?)
    }
    /// Roll back or delete only exact owned stages/backups and checked-close outputs.
    pub(crate) fn retire(&mut self, proof: &ReportExportRetirement) -> Result<(), E> {
        let fields = self.fields.as_mut().ok_or(E::Rejected(C::Identity))?;
        lifecycle::retire(fields, proof)
    }
    /// Observe absence of every physical output after checked native close.
    pub(crate) fn closed(&self) -> bool {
        self.fields.as_ref().is_none_or(|f| {
            f.stage.handle.is_none()
                && f.target.handle.is_none()
                && f.parents.iter().all(|slot| slot.handle.is_none())
        })
    }
    /// Observe held outputs and accepted activation without granting OS access.
    pub(crate) fn custody(&self) -> (u16, bool, bool, bool) {
        self.fields.as_ref().map_or((0, false, false, false), |fields| {
            let stage = fields.stage.handle.is_some();
            let predecessor = fields.target.handle.is_some();
            let parents = fields.parents.iter().filter(|slot| slot.handle.is_some()).count();
            (
                (parents + usize::from(stage) + usize::from(predecessor)) as u16,
                stage,
                predecessor,
                fields.stage_activated,
            )
        })
    }
}
impl Drop for Destination {
    fn drop(&mut self) {
        if !self.closed()
            && let Some(fields) = self.fields.take()
        {
            // An abandoned armed owner cannot release accounting as evidence of close.
            // The finite service retains this owner until checked cleanup; a programming
            // error pins its admission rather than falsely declaring outputs retired.
            std::mem::forget(fields);
        }
    }
}
// SAFETY: Handles are synchronous kernel files; all effects require one mutable owner
// and genuine running/cleanup controls. No window-affine or callback handle is retained.
unsafe impl Send for Destination {}
const _: () = assert!(size_of::<Fields>() < 32_768);
