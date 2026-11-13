// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A shortage reclaims before it is reported, and it is told apart from every other failure.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::cell::Cell;

/// Free space that grows by what each reclaim frees.
struct Volume {
    free: Cell<u64>,
    queries: Cell<usize>,
}
impl FreeSpace for Volume {
    fn available(&self, _: &Path) -> io::Result<u64> {
        self.queries.set(self.queries.get() + 1);
        Ok(self.free.get())
    }
}
struct Frees<'a>(&'a Volume, u64, usize);
impl Reclaim for Frees<'_> {
    fn reclaim(&mut self, _: &JobContext) -> io::Result<()> {
        self.2 += 1;
        self.0.free.set(self.0.free.get() + self.1);
        Ok(())
    }
}
fn volume(free: u64) -> Volume {
    Volume { free: Cell::new(free), queries: Cell::new(0) }
}

#[test]
fn enough_space_needs_no_reclaim_and_nothing_to_write_needs_no_space() -> TestResult {
    worker::run(|context, _| {
        let space = volume(HEADROOM_BYTES + 10);
        let mut reclaim = Frees(&space, 0, 0);
        ensure(Path::new("C:\\"), 10, &space, &mut [&mut reclaim], context)?;
        let full = volume(0);
        ensure(Path::new("C:\\"), 0, &full, &mut [&mut reclaim], context)?;
        assert_eq!(reclaim.2, 0);
        Ok(())
    })
}

#[test]
fn a_shortage_reclaims_first_and_succeeds_when_reclaim_made_room() -> TestResult {
    worker::run(|context, _| {
        let space = volume(HEADROOM_BYTES);
        let mut first = Frees(&space, 4, 0);
        let mut second = Frees(&space, 6, 0);
        ensure(Path::new("C:\\"), 10, &space, &mut [&mut first, &mut second], context)?;
        assert_eq!((first.2, second.2, space.queries.get()), (1, 1, 2));
        Ok(())
    })
}

#[test]
fn a_shortage_that_remains_is_typed_with_what_is_needed_and_free() -> TestResult {
    worker::run(|context, _| {
        let space = volume(HEADROOM_BYTES);
        let mut reclaim = Frees(&space, 3, 0);
        let Err(cause) = ensure(Path::new("C:\\"), 10, &space, &mut [&mut reclaim], context) else {
            return Err("a shortage passed".into());
        };
        assert_eq!(cause.kind(), io::ErrorKind::StorageFull);
        let shortage = SpaceShortage::find(&cause).ok_or("the shortage is not typed")?;
        assert_eq!(
            shortage,
            SpaceShortage { needed: HEADROOM_BYTES + 10, available: HEADROOM_BYTES + 3 }
        );
        assert!(shortage.to_string().starts_with("Not enough free disk space"));
        let wrapped = io::Error::other(cause);
        assert_eq!(SpaceShortage::find(&wrapped), Some(shortage), "found through wrappers");
        let budget = io::Error::new(io::ErrorKind::StorageFull, "Cache entry byte reservation");
        assert_eq!(SpaceShortage::find(&budget), None);
        Ok(())
    })
}

#[test]
fn the_volume_reports_free_space_for_a_real_directory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let free = VolumeSpace.available(directory.path())?;
    assert!(free > 0, "a writable temporary directory has free space");
    assert!(VolumeSpace.available(&directory.path().join("missing")).is_err());
    Ok(())
}
