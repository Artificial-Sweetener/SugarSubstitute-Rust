// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A full disk is a typed shortage: checked before writing, reclaimed first, kept apart from the
//! cache's own budget, and the received prefix survives for a later attempt.
use super::*;
use crate::setup_storage::{FreeSpace, HEADROOM_BYTES, Reclaim, SpaceShortage};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

/// A volume whose free space the test sets.
#[derive(Clone, Default)]
struct Volume(Arc<AtomicU64>);
impl FreeSpace for Volume {
    fn available(&self, _: &Path) -> io::Result<u64> {
        Ok(self.0.load(Ordering::Acquire))
    }
}
/// A reclaimer that frees a fixed number of bytes on the volume each time it runs.
struct Frees(Volume, u64, Arc<AtomicUsize>);
impl Reclaim for Frees {
    fn reclaim(&mut self, _: &JobContext) -> io::Result<()> {
        self.2.fetch_add(1, Ordering::AcqRel);
        self.0.0.fetch_add(self.1, Ordering::AcqRel);
        Ok(())
    }
}
/// A body that delivers a prefix, then reports the disk full as Windows does (ERROR_DISK_FULL).
struct FillsDisk(Cursor<Vec<u8>>);
impl Read for FillsDisk {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.0.read(output)?;
        if count == 0 { Err(io::Error::from_raw_os_error(112)) } else { Ok(count) }
    }
}

#[test]
fn a_short_disk_reclaims_first_then_refuses_before_writing_with_a_typed_shortage() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(230, context)?;
        let volume = Volume::default();
        let runs = Arc::new(AtomicUsize::new(0));
        let mut reclaim = Frees(volume.clone(), 1, Arc::clone(&runs));
        let Err(refused) = Download::open_with(
            storage(root.path(), context)?,
            target()?,
            Box::new(volume.clone()),
            &mut [&mut reclaim],
            context,
        ) else {
            return Err("a full disk admitted the download".into());
        };
        assert_eq!(runs.load(Ordering::Acquire), 1, "reclaim ran before the refusal");
        let needed = PACKAGE.len() as u64 + HEADROOM_BYTES;
        assert!(matches!(
            refused,
            DownloadFailure::InsufficientSpace(SpaceShortage { needed: n, available: 1 }) if n == needed
        ));
        assert_eq!(refused.space_shortage().map(|shortage| shortage.needed), Some(needed));
        assert!(std::fs::read(payload(root.path())?)?.is_empty(), "nothing was written");
        // Reclaim that frees enough admits the download.
        let mut enough = Frees(volume.clone(), needed, Arc::clone(&runs));
        let download = Download::open_with(
            storage(root.path(), context)?,
            target()?,
            Box::new(volume),
            &mut [&mut enough],
            context,
        )?;
        assert_eq!(download.offset()?, 0);
        Ok(())
    })
}

#[test]
fn a_disk_that_fills_mid_transfer_keeps_the_prefix_and_resumes_once_space_returns() -> TestResult {
    worker::run_reserved(2 * DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(231, context)?;
        let volume = Volume(Arc::new(AtomicU64::new(u64::MAX)));
        let prefix = 9;
        {
            let mut download = Download::open_with(
                storage(root.path(), context)?,
                target()?,
                Box::new(volume.clone()),
                &mut [],
                context,
            )?;
            volume.0.store(3, Ordering::Release);
            let failed = download.receive(
                full(),
                &mut FillsDisk(Cursor::new(PACKAGE[..prefix].to_vec())),
                context,
            );
            let expected = SpaceShortage {
                needed: (PACKAGE.len() - prefix) as u64 + HEADROOM_BYTES,
                available: 3,
            };
            assert!(
                matches!(failed, Err(DownloadFailure::InsufficientSpace(shortage)) if shortage == expected),
                "{failed:?}"
            );
            assert_eq!(download.offset()?, prefix as u64, "the received prefix was kept");
        }
        volume.0.store(u64::MAX, Ordering::Release);
        let mut download = Download::open_with(
            storage(root.path(), context)?,
            target()?,
            Box::new(volume),
            &mut [],
            context,
        )?;
        assert_eq!(download.offset()?, prefix as u64);
        let range = format!("bytes {prefix}-{}/{}", PACKAGE.len() - 1, PACKAGE.len());
        download.receive(
            Response {
                status: 206,
                length: Some((PACKAGE.len() - prefix) as u64),
                range: Some(&range),
                encoding: None,
            },
            &mut Cursor::new(&PACKAGE[prefix..]),
            context,
        )?;
        let mut verified = download.finish(|| NOW, context)?;
        let mut actual = Vec::new();
        verified.read_to_end(&mut actual)?;
        assert_eq!(actual, PACKAGE);
        Ok(())
    })
}

#[test]
fn the_cache_budget_and_a_full_disk_stay_different_failures() {
    let budget = crate::setup_storage::cache::CacheBudgetExhausted::io(
        "Cache entry byte reservation exhausted",
    );
    assert!(matches!(DownloadFailure::from(budget), DownloadFailure::CacheBudget(_)));
    let shortage = SpaceShortage { needed: 10, available: 2 };
    assert!(matches!(
        DownloadFailure::from(io::Error::other(shortage.into_io())),
        DownloadFailure::InsufficientSpace(found) if found == shortage
    ));
    let other = DownloadFailure::from(io::Error::from(io::ErrorKind::PermissionDenied));
    assert!(matches!(other, DownloadFailure::Storage(_)));
    assert_eq!(other.space_shortage(), None);
    let flushed = DownloadFailure::Flush {
        transfer: Box::new(DownloadFailure::InsufficientSpace(shortage)),
        flush: io::Error::from_raw_os_error(112),
    };
    assert_eq!(flushed.space_shortage(), Some(shortage));
}
