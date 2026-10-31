// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify failed admission precedes decoder reads and controls survive swallowed byte errors.
use super::*;
use std::{
    io::{BufRead, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct ObservedInput<'a> {
    input: Cursor<&'static [u8]>,
    reads: Arc<AtomicUsize>,
    pause: Option<&'a harness::CancelPoint>,
    armed: Arc<AtomicBool>,
}
impl Read for ObservedInput<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.armed.swap(false, Ordering::SeqCst)
            && let Some(point) = self.pause
        {
            point.pause();
        }
        self.input.read(output)
    }
}
impl BufRead for ObservedInput<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.input.fill_buf()
    }
    fn consume(&mut self, amount: usize) {
        self.input.consume(amount);
    }
}
impl Seek for ObservedInput<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.input.seek(from)
    }
}

#[test]
fn jpeg_insufficient_admission_performs_no_decoder_reads_or_output_writes() -> harness::TestResult {
    // First fail the scratch claim; then admit scratch exactly but deny the RGB allocation.
    for budget in [4096, 15_816] {
        harness::run(budget, move |context| {
            let reads = Arc::new(AtomicUsize::new(0));
            let input = ObservedInput {
                input: Cursor::new(FIXTURES[3]),
                reads: reads.clone(),
                pause: None,
                armed: Arc::new(AtomicBool::new(false)),
            };
            let import = JpegImport::open(input, [0; 32], context)?;
            reads.store(0, Ordering::SeqCst);
            let mut output = Vec::new();
            assert!(matches!(import.write(&mut output, context), Err(ImportError::Image(_))));
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            assert!(output.is_empty());
            drop(context.memory().claim(budget)?);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn jpeg_large_valid_dimensions_fail_admission_before_proportional_allocation() -> harness::TestResult
{
    harness::run(1024 * 1024, |context| {
        let mut bytes = FIXTURES[3].to_vec();
        let sof = marker(&bytes, 0xc0)?;
        bytes[sof + 5..sof + 7].copy_from_slice(&2048_u16.to_be_bytes());
        bytes[sof + 7..sof + 9].copy_from_slice(&4096_u16.to_be_bytes());
        let import = JpegImport::open(Cursor::new(bytes), [0; 32], context)?;
        assert_eq!(import.descriptor().layout().extent(), Extent::new(4096, 2048)?);
        assert!(matches!(import.write(&mut Vec::new(), context), Err(ImportError::Image(_))));
        drop(context.memory().claim(1024 * 1024)?);
        Ok(())
    })
}

#[test]
fn jpeg_preflight_exact_read_cancellation_is_terminal_without_interrupted_retries()
-> harness::TestResult {
    let observed = Arc::new(AtomicBool::new(false));
    let worker = observed.clone();
    harness::run_cancelled(1024 * 1024, move |context, point| {
        let input = ObservedInput {
            input: Cursor::new(FIXTURES[3]),
            reads: Arc::new(AtomicUsize::new(0)),
            pause: Some(point),
            armed: Arc::new(AtomicBool::new(true)),
        };
        let stopped = matches!(JpegImport::open(input, [0; 32], context), Err(ImportError::Io(ref cause)) if cause.kind() == io::ErrorKind::Interrupted);
        worker.store(stopped, Ordering::SeqCst);
        Ok(())
    })?;
    assert!(observed.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn jpeg_decode_read_cancellation_writes_no_archive_and_releases_admission() -> harness::TestResult {
    let observed = Arc::new(AtomicBool::new(false));
    let worker = observed.clone();
    harness::run_cancelled(2 * 1024 * 1024, move |context, point| {
        let armed = Arc::new(AtomicBool::new(false));
        let input = ObservedInput {
            input: Cursor::new(FIXTURES[3]),
            reads: Arc::new(AtomicUsize::new(0)),
            pause: Some(point),
            armed: armed.clone(),
        };
        let import = JpegImport::open(input, [0; 32], context)?;
        armed.store(true, Ordering::SeqCst);
        let mut output = Vec::new();
        let stopped = matches!(import.write(&mut output, context), Err(ImportError::Io(ref cause)) if cause.kind() == io::ErrorKind::Interrupted);
        worker.store(
            stopped && output.is_empty() && context.memory().claim(2 * 1024 * 1024).is_ok(),
            Ordering::SeqCst,
        );
        Ok(())
    })?;
    assert!(observed.load(Ordering::SeqCst));
    Ok(())
}
