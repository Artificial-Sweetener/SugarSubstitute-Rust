// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A decoder-pass failure must survive zune's speculative read fallback with its original cause.
use super::*;
use std::{
    fmt,
    io::{BufRead, Read, Seek, SeekFrom},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug)]
struct StorageFailure;
impl fmt::Display for StorageFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one-shot JPEG storage failure")
    }
}
impl std::error::Error for StorageFailure {}

struct FaultInput {
    input: Cursor<&'static [u8]>,
    armed: Arc<AtomicBool>,
}
impl Read for FaultInput {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.len() == 4 && self.armed.swap(false, Ordering::SeqCst) {
            return Err(io::Error::other(StorageFailure));
        }
        self.input.read(output)
    }
}
impl BufRead for FaultInput {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.input.fill_buf()
    }
    fn consume(&mut self, amount: usize) {
        self.input.consume(amount);
    }
}
impl Seek for FaultInput {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.input.seek(from)
    }
}

#[test]
fn jpeg_speculative_decoder_read_failure_keeps_its_original_storage_cause() -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        let armed = Arc::new(AtomicBool::new(false));
        let input = FaultInput { input: Cursor::new(FIXTURES[3]), armed: armed.clone() };
        let import = JpegImport::open(input, [0; 32], context)?;
        armed.store(true, Ordering::SeqCst);
        let mut output = Vec::new();
        let failure = import
            .write(&mut output, context)
            .err()
            .ok_or(ImportError::Invalid("JPEG swallowed the original storage failure"))?;
        assert!(
            matches!(failure, ImportError::Io(ref cause) if cause.get_ref().is_some_and(|cause| cause.is::<StorageFailure>()))
        );
        assert!(!armed.load(Ordering::SeqCst));
        assert!(output.is_empty());
        drop(context.memory().claim(2 * 1024 * 1024)?);
        Ok(())
    })
}
