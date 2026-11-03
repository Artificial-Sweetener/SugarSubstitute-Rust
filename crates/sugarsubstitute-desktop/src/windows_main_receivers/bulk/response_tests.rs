// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read a staged message through the actual COM stream and observe its claim release.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use windows::Win32::UI::Shell::SHCreateMemStream;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

fn stream(lease: &MemoryLease, bytes: Vec<u8>) -> Result<IStream, Box<dyn Error>> {
    let claim = lease.claim(bytes.len() as u64)?;
    Ok(Body { bytes, position: AtomicU64::new(0), _claim: claim }.into())
}

/// Read the remaining stream in caller-sized native reads.
fn drain(stream: &IStream, size: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut output = Vec::new();
    let mut chunk = vec![0_u8; size as usize];
    loop {
        let mut read = 0_u32;
        // SAFETY: `chunk` is owned writable storage of `size` bytes and `read` is a live local.
        let status = unsafe { stream.Read(chunk.as_mut_ptr().cast(), size, Some(&mut read)) };
        status.ok()?;
        output.extend_from_slice(&chunk[..read as usize]);
        if read < size {
            return Ok(output);
        }
    }
}

#[test]
fn staged_bytes_read_exactly_once_and_release_their_claim_with_the_last_reference()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(8 * 1024 * 1024, Box::new(Release));
    let body: Vec<u8> = (0..5 * 1024 * 1024 + 7).map(|index| (index % 251) as u8).collect();
    let stream = stream(&lease, body.clone())?;
    assert!(lease.claim(4 * 1024 * 1024).is_err());
    let mut stat = STATSTG::default();
    // SAFETY: `stat` is a live writable output for this synchronous call.
    unsafe { stream.Stat(&mut stat, STATFLAG_NONAME) }?;
    assert_eq!(stat.cbSize, body.len() as u64);
    assert_eq!(drain(&stream, 65536)?, body);
    assert!(drain(&stream, 65536)?.is_empty());
    let mut position = 0;
    // SAFETY: Live stream on this thread with a writable optional output.
    unsafe { stream.Seek(-7, STREAM_SEEK_END, Some(&mut position)) }?;
    assert_eq!(position, body.len() as u64 - 7);
    assert_eq!(drain(&stream, 3)?, body[body.len() - 7..]);
    // SAFETY: Live stream on this thread; a negative absolute position is invalid.
    assert!(unsafe { stream.Seek(-1, STREAM_SEEK_SET, None) }.is_err());
    let mut written = 1_u32;
    // SAFETY: The source byte and count output are live locals for this call.
    let status = unsafe { stream.Write([1_u8].as_ptr().cast(), 1, Some(&mut written)) };
    assert_eq!(status, STG_E_ACCESSDENIED);
    assert_eq!(written, 0);
    drop(stream);
    assert!(lease.claim(8 * 1024 * 1024).is_ok());
    Ok(())
}

#[test]
fn copy_to_transfers_the_remaining_bytes_into_another_stream() -> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let body: Vec<u8> = (0..3 * 1024 * 1024 + 11).map(|index| (index % 239) as u8).collect();
    let source = stream(&lease, body.clone())?;
    // SAFETY: Creates an empty independent memory stream with no borrowed input.
    let target = unsafe { SHCreateMemStream(None) }.ok_or("Missing memory stream")?;
    let (mut read, mut written) = (0, 0);
    // SAFETY: Both live streams belong to this thread; outputs are live locals.
    unsafe { source.CopyTo(&target, u64::MAX, Some(&mut read), Some(&mut written)) }?;
    assert_eq!((read, written), (body.len() as u64, body.len() as u64));
    // SAFETY: Rewind the live target stream before reading it back.
    unsafe { target.Seek(0, STREAM_SEEK_SET, None) }?;
    assert_eq!(drain(&target, 65536)?, body);
    Ok(())
}

#[test]
fn system_tokens_are_distinct_draws() -> Result<(), Box<dyn Error>> {
    let first = super::super::random_token()?;
    let second = super::super::random_token()?;
    assert_ne!(first, second);
    Ok(())
}
