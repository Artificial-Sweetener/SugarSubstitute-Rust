// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact read admission and primary/post-control custody at the I/O boundary.
use super::*;
use std::{
    cell::Cell,
    error::Error,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
type ResultT = Result<(), Box<dyn Error>>;

/// Exercise the exact allocation and control profile used by admitted preparation.
fn prepared<C>(
    source: impl Read,
    maximum: usize,
    before: impl FnMut() -> Result<(), C>,
    after: impl FnMut() -> Result<(), C>,
) -> Result<Vec<u8>, ReadFailure<C>> {
    bounded(source, maximum, Allocation::Exact, InterruptedReads::Propagate, before, after)
}

#[test]
fn preparation_input_exact_zero_and_sentinel_bounds_preserve_fixed_capacity() -> ResultT {
    const LIMIT: usize = 64 * 1024;
    for length in [0, 1, LIMIT] {
        let bytes = prepared(Cursor::new(vec![b'x'; length]), LIMIT, || Ok::<_, ()>(()), || Ok(()))
            .map_err(|_| "Bounded read rejected")?;
        assert_eq!(bytes.len(), length);
        assert_eq!(bytes.capacity(), LIMIT + 1);
    }
    assert!(matches!(
        prepared(Cursor::new(vec![0; LIMIT + 1]), LIMIT, || Ok::<_, ()>(()), || Ok(())),
        Err(ReadFailure::TooLarge)
    ));
    let empty = prepared(Cursor::new([]), 0, || Ok::<_, ()>(()), || Ok(()))
        .map_err(|_| "Empty bounded read rejected")?;
    assert!(empty.is_empty());
    assert_eq!(empty.capacity(), 1);
    assert!(matches!(
        prepared(Cursor::new([1]), 0, || Ok::<_, ()>(()), || Ok(())),
        Err(ReadFailure::TooLarge)
    ));
    assert!(matches!(
        prepared(Cursor::new([]), usize::MAX, || Ok::<_, ()>(()), || Ok(())),
        Err(ReadFailure::TooLarge)
    ));
    Ok(())
}

#[test]
fn preparation_input_keeps_primary_io_cause_through_lazy_post_failure() -> ResultT {
    struct Fails;
    impl Read for Fails {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "original read"))
        }
    }
    let checks = Cell::new(0);
    let result = prepared(
        Fails,
        16,
        || Ok::<_, ()>(()),
        || {
            checks.set(checks.get() + 1);
            Err(())
        },
    );
    let Err(ReadFailure::Read { source, after: Some(()) }) = result else {
        return Err("First read error disappeared".into());
    };
    assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(checks.get(), 1);
    Ok(())
}

#[test]
fn preparation_input_pre_and_post_rejections_stop_further_reads() {
    struct Observed<'a>(&'a Cell<usize>);
    impl Read for Observed<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.0.set(self.0.get() + 1);
            bytes[0] = 1;
            Ok(1)
        }
    }
    let calls = Cell::new(0);
    assert!(matches!(
        prepared(Observed(&calls), 16, || Err(()), || Ok(())),
        Err(ReadFailure::Control(()))
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        prepared(Observed(&calls), 16, || Ok(()), || Err(())),
        Err(ReadFailure::Control(()))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn preparation_input_owns_failed_open_before_mandatory_observation() -> ResultT {
    #[derive(Debug)]
    struct Original(Arc<AtomicBool>);
    impl std::fmt::Display for Original {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("original")
        }
    }
    impl Error for Original {}
    impl Drop for Original {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let outcome: io::Result<()> = Err(io::Error::other(Original(dropped.clone())));
    let result = observe(outcome, || {
        assert!(!dropped.load(Ordering::Acquire));
        Err("post")
    });
    let Err(ReadFailure::Read { source, after: Some("post") }) = result else {
        return Err("Open cause or post-control disappeared".into());
    };
    assert!(!dropped.load(Ordering::Acquire));
    assert!(source.get_ref().is_some_and(|cause| cause.is::<Original>()));
    drop(source);
    assert!(dropped.load(Ordering::Acquire));
    Ok(())
}

#[test]
fn preparation_input_observes_eof_control_and_rejects_invalid_returned_extent() {
    assert!(matches!(
        prepared(Cursor::new([]), 16, || Ok(()), || Err("eof")),
        Err(ReadFailure::Control("eof"))
    ));
    struct InvalidExtent;
    impl Read for InvalidExtent {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Ok(usize::MAX)
        }
    }
    let checks = Cell::new(0);
    assert!(matches!(
        prepared(
            InvalidExtent,
            16,
            || Ok::<_, ()>(()),
            || {
                checks.set(checks.get() + 1);
                Ok(())
            }
        ),
        Err(ReadFailure::TooLarge)
    ));
    assert_eq!(checks.get(), 1);
}

#[test]
fn preparation_input_retains_interrupted_read_as_original_failure() -> ResultT {
    struct Interrupted<'a>(&'a Cell<usize>);
    impl Read for Interrupted<'_> {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            self.0.set(self.0.get() + 1);
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    let calls = Cell::new(0);
    let result = prepared(Interrupted(&calls), 16, || Ok::<_, ()>(()), || Ok(()));
    let Err(ReadFailure::Read { source, after: None }) = result else {
        return Err("Interrupted read was retried or lost".into());
    };
    assert_eq!(source.kind(), io::ErrorKind::Interrupted);
    assert_eq!(calls.get(), 1);
    Ok(())
}

#[test]
fn ordinary_record_large_bound_keeps_incremental_allocation() -> ResultT {
    let bytes = bounded(
        Cursor::new(b"record"),
        usize::MAX - 1,
        Allocation::Incremental { observed: 6 },
        InterruptedReads::Resume,
        || Ok::<_, ()>(()),
        || Ok(()),
    )
    .map_err(|_| "Small record rejected under a large upper bound")?;
    assert_eq!(bytes, b"record");
    assert_eq!(bytes.capacity(), 7);
    assert!(matches!(
        bounded(
            Cursor::new([]),
            1,
            Allocation::Incremental { observed: 2 },
            InterruptedReads::Resume,
            || Ok::<_, ()>(()),
            || Ok(())
        ),
        Err(ReadFailure::TooLarge)
    ));
    Ok(())
}

#[test]
fn ordinary_record_growth_uses_the_same_sentinel_bound() -> ResultT {
    for length in [0, 64, 8192, 8193, 20_000] {
        let bytes = bounded(
            Cursor::new(vec![7; length]),
            20_000,
            Allocation::Incremental { observed: 0 },
            InterruptedReads::Resume,
            || Ok::<_, ()>(()),
            || Ok(()),
        )
        .map_err(|_| "Record growth below its bound was rejected")?;
        assert_eq!(bytes, vec![7; length]);
        assert!(bytes.capacity() <= 20_001);
    }
    assert!(matches!(
        bounded(
            Cursor::new(vec![0; 20_001]),
            20_000,
            Allocation::Incremental { observed: 0 },
            InterruptedReads::Resume,
            || Ok::<_, ()>(()),
            || Ok(())
        ),
        Err(ReadFailure::TooLarge)
    ));
    Ok(())
}

#[test]
fn ordinary_record_resumes_interruption_with_observed_controls() -> ResultT {
    struct Once {
        interrupted: bool,
        source: Cursor<&'static [u8]>,
    }
    impl Read for Once {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.source.read(output)
        }
    }
    let checks = Cell::new(0);
    let bytes = bounded(
        Once { interrupted: false, source: Cursor::new(b"record") },
        16,
        Allocation::Incremental { observed: 6 },
        InterruptedReads::Resume,
        || Ok::<_, ()>(()),
        || {
            checks.set(checks.get() + 1);
            Ok(())
        },
    )
    .map_err(|_| "Ordinary interrupted record did not resume")?;
    assert_eq!(bytes, b"record");
    assert_eq!(checks.get(), 3);
    Ok(())
}

#[test]
fn interrupted_record_retains_primary_and_failed_post_control() -> ResultT {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    let result = bounded(
        Interrupted,
        16,
        Allocation::Incremental { observed: 0 },
        InterruptedReads::Resume,
        || Ok(()),
        || Err("post"),
    );
    let Err(ReadFailure::Read { source, after: Some("post") }) = result else {
        return Err("Record resumed past its failed mandatory observation".into());
    };
    assert_eq!(source.kind(), io::ErrorKind::Interrupted);
    Ok(())
}

#[test]
fn impossible_preparation_allocation_preserves_actual_reservation_failure() -> ResultT {
    use crate::durable_storage::input::PreparationFileError;
    use sugarsubstitute_execution::EntryPreparationError;
    let failure = bounded(
        Cursor::new([]),
        usize::MAX - 1,
        Allocation::Exact,
        InterruptedReads::Propagate,
        || Ok::<_, EntryPreparationError>(()),
        || Ok(()),
    )
    .err()
    .ok_or("Impossible allocation unexpectedly succeeded")?;
    let cause = PreparationFileError::from(failure);
    assert!(matches!(cause, PreparationFileError::Capacity(_)));
    assert!(cause.source().is_some_and(|source| source.is::<std::collections::TryReserveError>()));
    Ok(())
}
