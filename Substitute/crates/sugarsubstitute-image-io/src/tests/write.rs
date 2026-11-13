// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise complete archive writes under partial progress, interruption and original failures.

use super::*;
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;

#[cfg(windows)]
mod cache;
mod deadline;

fn descriptor() -> Result<RasterDescriptor, ImportError> {
    RasterDescriptor::new(Extent::new(2, 1)?, Packing::Rgba8, Transfer::Srgb, [91; 32])
}

fn archive(output: &mut dyn Write, context: &JobContext) -> Result<(), ImportError> {
    write_raster(output, descriptor()?, context, |_, row| {
        row.copy_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255]);
        Ok(())
    })
}

struct PartialInterrupted {
    bytes: Vec<u8>,
    interrupted: usize,
    next_interrupt: bool,
}
impl Write for PartialInterrupted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupted != 0 || self.next_interrupt {
            self.interrupted = self.interrupted.saturating_sub(1);
            self.next_interrupt = false;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let count = bytes.len().min(3);
        self.bytes.extend_from_slice(&bytes[..count]);
        self.next_interrupt = true;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn archive_partial_writes_and_transient_interruptions_preserve_every_header_hash_and_sample_byte()
-> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let mut expected = Vec::new();
        archive(&mut expected, context)?;
        let mut output =
            PartialInterrupted { bytes: Vec::new(), interrupted: 128, next_interrupt: true };
        archive(&mut output, context)?;
        assert_eq!(output.bytes, expected);
        assert_eq!(output.bytes.len() as u64, descriptor()?.layout().length());
        Ok(())
    })
}

#[derive(Debug)]
struct NativeCause(Arc<()>);
impl fmt::Display for NativeCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Original image output failure")
    }
}
impl Error for NativeCause {}

struct FailedOutput(Option<io::Error>);
impl Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.0.take().map_or(Ok(0), Err)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn archive_output_failure_preserves_its_identical_original_native_cause() -> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let identity = Arc::new(());
        let original =
            io::Error::new(io::ErrorKind::PermissionDenied, NativeCause(identity.clone()));
        let error = archive(&mut FailedOutput(Some(original)), context)
            .err()
            .ok_or(ImportError::Invalid("Failed archive output completed"))?;
        let ImportError::Io(cause) = error else {
            return Err(ImportError::Invalid("Archive output replaced the native failure"));
        };
        assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
        let original = cause
            .get_ref()
            .and_then(|cause| cause.downcast_ref::<NativeCause>())
            .ok_or(ImportError::Invalid("Archive output lost the original native cause"))?;
        assert!(Arc::ptr_eq(&identity, &original.0));
        Ok(())
    })
}

#[test]
fn archive_writer_zero_progress_reports_write_zero_without_retrying() -> harness::TestResult {
    harness::run(1024 * 1024, |context| {
        let error = archive(&mut FailedOutput(None), context)
            .err()
            .ok_or(ImportError::Invalid("Zero-progress archive completed"))?;
        assert!(matches!(error, ImportError::Io(cause) if cause.kind()==io::ErrorKind::WriteZero));
        Ok(())
    })
}
