// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold exclusion as byte-range slots on a shared rendezvous, so foreign readers never block it.
//!
//! The rendezvous is opened with read and write sharing, so an indexer, backup or antivirus
//! handle on it cannot make the owner look busy. Exclusion is a lock on one slot byte past the
//! empty file's end. Windows releases every byte-range lock a process holds on a file object when
//! that process closes any handle to the object, so a holder locks through a private file object
//! it never duplicates, and hands other processes a separate witness object.
//!
//! The gate serializes fresh acquisition: a fresh acquirer takes the gate, proves every slot
//! free, takes the first slot and releases the gate. A sender holds the handoff byte while a
//! receiver adopts; the receiver takes any free slot and then requires the handoff byte to still
//! be held, so the sender's slot was held continuously until the receiver held its own. The
//! handoff byte is locked through a third, overlapped file object of the holder, so a sender can
//! wait in the kernel for an earlier sender's handoff to end and can release its own handoff
//! while keeping its slot. That object always closes before the slot's object, so a held handoff
//! byte always means its holder still holds a slot.
//!
//! The byte layout is frozen surface: engines and applications of every generation contend on
//! the same rendezvous files, so these offsets never change (`native-exclusion-slots-v1`).
use super::*;
use std::{fmt, os::windows::io::AsHandle, sync::Mutex};
use sugarsubstitute_contracts::execution::Moment;
use sugarsubstitute_execution::file_lock::{self, FileLockError, LockKind};
use windows::Win32::{
    Foundation::{ERROR_LOCK_VIOLATION, HANDLE},
    Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx, ReOpenFile, UnlockFileEx,
    },
    System::IO::OVERLAPPED,
};

/// Offsets lie far past the end of the always-empty rendezvous; no data is ever covered.
const GATE: u64 = 1 << 40;
const HANDOFF: u64 = GATE + 1;
const FIRST_SLOT: u64 = GATE + 2;
/// Bounds simultaneous holders in one handoff chain (launcher, application, custody helper).
const SLOTS: u64 = 8;

/// Another live process holds this exclusion; carried as `io::ErrorKind::WouldBlock`.
#[derive(Debug)]
pub(crate) struct ExclusionBusy;
impl fmt::Display for ExclusionBusy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Another process holds this exclusion")
    }
}
impl std::error::Error for ExclusionBusy {}

/// Whether a lock owner reported live contention rather than a storage failure.
pub(crate) fn is_busy(cause: &io::Error) -> bool {
    cause.kind() == io::ErrorKind::WouldBlock
        && cause.get_ref().is_some_and(|inner| inner.is::<ExclusionBusy>())
}
/// Typed contention for every exclusion owner in this module.
pub(super) fn busy() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, ExclusionBusy)
}

/// Live exclusion held through a private lock object; only the witness may cross processes.
/// Dropping it closes the handoff object first and then the private object, which releases the
/// slot, so the handoff byte is never held without a slot.
pub(crate) struct SlotExclusion {
    handoff: Mutex<Option<File>>,
    lock: File,
    witness: File,
}
impl SlotExclusion {
    fn new(lock: File, witness: File) -> Self {
        Self { handoff: Mutex::new(None), lock, witness }
    }

    /// Separate handle on the same rendezvous for duplication into a receiving process. Closing
    /// any duplicate of it never releases this exclusion.
    pub(crate) fn witness(&self) -> &File {
        &self.witness
    }

    /// Hold the handoff byte so a receiver may adopt this exclusion, waiting in the kernel until
    /// `until` on the job's clock while an earlier sender in this chain still holds it; pass the
    /// job's current time to refuse at once instead. Keep it until the receiver holds its own
    /// slot or is known to be gone, then `release_handoff`; dropping this owner also releases
    /// it. Repeated holds are idempotent. Run on a blocking infrastructure job.
    /// # Errors
    /// `is_busy` while another holder's handoff lasts past `until`; `Interrupted` on
    /// cancellation; storage errors otherwise.
    pub(crate) fn hold_handoff(&self, context: &JobContext, until: Moment) -> io::Result<()> {
        let mut handoff = crate::native_claims::lock(&self.handoff);
        if handoff.is_some() {
            return Ok(());
        }
        let object = reopen_overlapped(&self.lock)?;
        file_lock::lock_range(
            context,
            object.as_handle(),
            LockKind::Exclusive,
            file_lock::Range { offset: HANDOFF, length: 1 },
            until,
        )
        .map_err(|cause| match cause {
            FileLockError::Timeout => busy(),
            FileLockError::Cancelled => io::Error::new(io::ErrorKind::Interrupted, cause),
            FileLockError::Io(cause) => cause,
            FileLockError::Unavailable => io::Error::other(cause),
        })?;
        *handoff = Some(object);
        Ok(())
    }

    /// End this holder's handoff: a receiver that has not yet confirmed it now refuses, while
    /// this holder keeps its own slot. Releasing a handoff that is not held does nothing.
    pub(crate) fn release_handoff(&self) {
        drop(crate::native_claims::lock(&self.handoff).take());
    }
}
impl Drop for SlotExclusion {
    fn drop(&mut self) {
        // Field order already closes the handoff object first; keep it explicit.
        self.release_handoff();
    }
}

/// A new file object on the same file for overlapped locking only. It requests read access,
/// which a lock needs, and shares reading and writing like every rendezvous handle.
fn reopen_overlapped(file: &File) -> io::Result<File> {
    // SAFETY: the borrowed handle is live for this call; the returned handle is unique and
    // adopted immediately.
    let raw = unsafe {
        ReOpenFile(
            HANDLE(file.as_raw_handle()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_FLAG_OVERLAPPED,
        )
    }
    .map_err(io::Error::other)?;
    // SAFETY: ReOpenFile returned this unique valid handle.
    let reopened = unsafe { File::from_raw_handle(raw.0) };
    same_file(file, &reopened)?;
    Ok(reopened)
}

/// One exclusively locked byte range on a synchronous handle, unlocked when dropped unless kept.
struct Range<'a> {
    file: &'a File,
    offset: u64,
    length: u64,
    kept: bool,
}
impl<'a> Range<'a> {
    /// Fail immediately on contention with `busy()`; never waits.
    fn try_lock(file: &'a File, offset: u64, length: u64) -> io::Result<Self> {
        let mut overlapped = at(offset);
        // SAFETY: the handle is synchronous and owned by `file` for this call. With
        // LOCKFILE_FAIL_IMMEDIATELY the request completes before returning, so `overlapped`
        // only needs to outlive the call.
        let result = unsafe {
            LockFileEx(
                HANDLE(file.as_raw_handle()),
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                None,
                length as u32,
                (length >> 32) as u32,
                &mut overlapped,
            )
        };
        match result {
            Ok(()) => Ok(Self { file, offset, length, kept: false }),
            Err(cause) if cause.code() == ERROR_LOCK_VIOLATION.to_hresult() => Err(busy()),
            Err(cause) => Err(io::Error::other(cause)),
        }
    }
    /// Keep the range until the lock object's handle closes.
    fn keep(mut self) {
        self.kept = true;
    }
}
impl Drop for Range<'_> {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        let mut overlapped = at(self.offset);
        // SAFETY: this exact range was locked through the same live handle by this owner.
        let unlocked = unsafe {
            UnlockFileEx(
                HANDLE(self.file.as_raw_handle()),
                None,
                self.length as u32,
                (self.length >> 32) as u32,
                &mut overlapped,
            )
        };
        if let Err(cause) = unlocked {
            // The range stays held until the handle closes; exclusion is never weakened.
            tracing::warn!(
                target: "sugarsubstitute::storage",
                operation = "exclusion_range_unlock",
                %cause,
                "Exclusion range stays held until its handle closes"
            );
        }
    }
}
fn at(offset: u64) -> OVERLAPPED {
    let mut overlapped = OVERLAPPED::default();
    overlapped.Anonymous.Anonymous.Offset = offset as u32;
    overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;
    overlapped
}

/// Require two handles to name the same native file.
fn same_file(left: &File, right: &File) -> io::Result<()> {
    if files::file_identity(left)? != files::file_identity(right)? {
        return Err(super::super::invalid("Exclusion rendezvous changed native identity"));
    }
    Ok(())
}

/// Whether any slot is held by another lock object, probed without keeping anything.
pub(super) fn slots_held(probe: &File) -> io::Result<bool> {
    match Range::try_lock(probe, FIRST_SLOT, SLOTS) {
        Ok(_released) => Ok(false),
        Err(cause) if is_busy(&cause) => Ok(true),
        Err(cause) => Err(cause),
    }
}

impl Protection {
    /// Acquire fresh exclusion on an empty protected rendezvous without waiting.
    /// The caller pins the parent. Foreign shared handles on the file never conflict.
    /// # Errors
    /// `is_busy` for a live holder; storage errors for unknown contents or protection.
    pub(crate) fn exclusion(&self, path: &Path, context: &JobContext) -> io::Result<SlotExclusion> {
        let lock = self.rendezvous(path, context)?;
        let witness = self.rendezvous(path, context)?;
        same_file(&lock, &witness)?;
        {
            let _gate = Range::try_lock(&lock, GATE, 1)?;
            drop(Range::try_lock(&lock, FIRST_SLOT, SLOTS)?);
            Range::try_lock(&lock, FIRST_SLOT, 1)?.keep();
        }
        check(context)?;
        Ok(SlotExclusion::new(lock, witness))
    }
}

impl Protection {
    /// Adopt exclusion transferred by a live sender that holds its handoff byte.
    /// Verifies the inherited witness, takes a free slot through a private lock object, then
    /// requires the sender to still hold the handoff byte; otherwise the slot is released.
    /// The returned owner keeps the exclusion after the sender exits.
    /// # Errors
    /// Rejects a foreign, unprotected or released rendezvous, a sender that already released its
    /// handoff, and a full slot table.
    pub(crate) fn adopt_exclusion(
        &self,
        inherited: File,
        path: &Path,
        context: &JobContext,
    ) -> io::Result<SlotExclusion> {
        self.verify_exclusive_rendezvous(&inherited, path, context)?;
        let lock = self.rendezvous(path, context)?;
        same_file(&lock, &inherited)?;
        let mut adopted = false;
        for slot in FIRST_SLOT..FIRST_SLOT + SLOTS {
            match Range::try_lock(&lock, slot, 1) {
                Ok(range) => {
                    range.keep();
                    adopted = true;
                    break;
                }
                Err(cause) if is_busy(&cause) => {}
                Err(cause) => return Err(cause),
            }
        }
        if !adopted {
            return Err(super::super::invalid("Every exclusion slot is held"));
        }
        match Range::try_lock(&lock, HANDOFF, 1) {
            Err(cause) if is_busy(&cause) => {}
            Ok(_released) => {
                return Err(super::super::invalid(
                    "The sender released this exclusion before handing it over",
                ));
            }
            Err(cause) => return Err(cause),
        }
        check(context)?;
        Ok(SlotExclusion::new(lock, inherited))
    }
}

/// Whether a live owner holds exclusion on the existing rendezvous at `path`. Opens the file
/// for reading only and never creates it.
/// # Errors
/// Missing files and native failures.
#[cfg(test)]
pub(crate) fn exclusion_held(path: &Path) -> io::Result<bool> {
    let probe = std::fs::File::open(path)?;
    slots_held(&probe)
}

#[cfg(test)]
mod tests;
