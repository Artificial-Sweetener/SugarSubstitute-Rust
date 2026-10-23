// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain user-selected receiving files under genuine export execution admission.
//!
//! The feature authenticates the request and authorizes overwrite before supplying an
//! inert selection. This owner performs all bounded path, identity, staging, publication
//! and physical close effects. An accepted native effect is adopted before the next
//! cancellation/deadline observation. Abandonment retains outputs and their admission;
//! dropping an owner never proves that a file or its cleanup job retired.
mod error;
mod files;
mod lifecycle;
mod names;
mod selection;
use self::{ReportFileCode as C, ReportFileError as E};
pub use error::{ReportFileCode, ReportFileError};
pub use selection::ReportFileSelection;
use sugarsubstitute_execution::{ReportExportOrigin, ReportExportOutput, ReportExportRetirement};
use windows::Win32::Foundation::HANDLE;
/// Couple one real kernel handle to its pre-reserved original output guard.
struct Slot {
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
struct Fields {
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
/// Own selected receiving files under original admitted export work through checked cleanup.
/// The opaque owner exposes passive custody facts and typed effects under its retained
/// original execution controls. File handles and unchecked publication stay private.
///
/// ```compile_fail
/// use sugarsubstitute_supervision::ReportFile;
/// fn detach(file: ReportFile) -> std::fs::File {
///     file.into_inner()
/// }
/// ```
#[must_use = "Retain receiving files through checked physical retirement"]
pub struct ReportFile {
    fields: Option<Box<Fields>>,
}
/// Passive receiving-file observations; these grant no native effect or close right.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReportFileCustody {
    /// Number of physical parent/file outputs still retained.
    pub native_outputs: u16,
    /// Receiving stage retains its exact kernel handle.
    pub stage: bool,
    /// Selected predecessor retains its exact kernel handle.
    pub predecessor: bool,
    /// Same-stage activation succeeded, independently of later cleanup.
    pub activated: bool,
}
impl ReportFile {
    /// Allocate the fixed destination envelope from genuine original admission.
    pub fn new(original: ReportExportOrigin) -> Self {
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
    /// # Errors
    /// Returns Identity when the original receiving owner is unavailable.
    pub fn origin(&self) -> Result<&ReportExportOrigin, E> {
        Ok(&self.fields.as_ref().ok_or(E::Rejected(C::Identity))?.original)
    }
    /// Pin selected ancestors and predecessor before creating the sole new stage.
    /// # Errors
    /// Rejects repeated open, uncertain ancestry, links, absent overwrite consent,
    /// failed native effects or expired original controls. Partial outputs stay retained.
    pub fn open(&mut self, intent: &ReportFileSelection) -> Result<(), E> {
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
    /// # Errors
    /// Rejects empty/oversized chunks, inactive stages, short native writes, OS errors
    /// or original control failure. Accepted byte counts remain observed.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), E> {
        files::write(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?, bytes)
    }
    /// Observe actual synchronous flush of the same retained receiving stage.
    /// # Errors
    /// Preserves the actual flush failure or original execution-control cause.
    pub fn flush(&mut self) -> Result<(), E> {
        files::flush(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?)
    }
    /// Preserve a held predecessor, then activate the stage with no replacement race.
    /// # Errors
    /// Rejects changed predecessor/stage identity, unexpected destination occupants,
    /// OS failures or expired controls. Accepted rename phases remain recorded.
    pub fn activate(&mut self) -> Result<(), E> {
        lifecycle::activate(self.fields.as_mut().ok_or(E::Rejected(C::Identity))?)
    }
    /// Roll back or delete only exact owned stages/backups and checked-close outputs.
    /// # Errors
    /// Rejects foreign/expired cleanup proof or native rollback/delete/close failure.
    /// Outputs and admission charges remain held until their exact close succeeds.
    pub fn retire(&mut self, proof: &ReportExportRetirement) -> Result<(), E> {
        let fields = self.fields.as_mut().ok_or(E::Rejected(C::Identity))?;
        lifecycle::retire(fields, proof)
    }
    /// Observe absence of every physical output after checked native close.
    pub fn closed(&self) -> bool {
        self.fields.as_ref().is_none_or(|f| {
            f.stage.handle.is_none()
                && f.target.handle.is_none()
                && f.parents.iter().all(|slot| slot.handle.is_none())
        })
    }
    /// Observe held outputs and accepted activation without granting OS access.
    pub fn custody(&self) -> ReportFileCustody {
        self.fields.as_ref().map_or_else(ReportFileCustody::default, |fields| {
            let stage = fields.stage.handle.is_some();
            let predecessor = fields.target.handle.is_some();
            let parents = fields.parents.iter().filter(|slot| slot.handle.is_some()).count();
            ReportFileCustody {
                native_outputs: (parents + usize::from(stage) + usize::from(predecessor)) as u16,
                stage,
                predecessor,
                activated: fields.stage_activated,
            }
        })
    }
}
impl Drop for ReportFile {
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
unsafe impl Send for ReportFile {}
const _: () = assert!(size_of::<Fields>() < 32_768);
