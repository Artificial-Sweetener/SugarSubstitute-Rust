// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact native image bounds, sticky failures and retained reservation custody.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::execution::MemoryRelease;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Released(Arc<AtomicU64>);
impl MemoryRelease for Released {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
fn lease(bytes: u64, released: &Arc<AtomicU64>) -> MemoryLease {
    MemoryLease::new(bytes, Box::new(Released(released.clone())))
}

#[test]
fn exact_capacity_write_freezes_and_moves_original_allocation_once() -> Result {
    let released = Arc::new(AtomicU64::new(0));
    let lease = lease(16, &released);
    let mut owner = Buffer::allocate(&lease, 16)?;
    let original = owner.bytes.as_ptr();
    owner.write(&[7; 16])?;
    owner.freeze()?;
    let image = owner.take()?;
    assert_eq!(image.bytes.as_ptr(), original);
    assert_eq!(image.bytes, [7; 16]);
    assert_eq!(image.bytes.capacity(), 16);
    assert_eq!(image.claim.bytes(), 16);
    assert!(matches!(owner.take(), Err(BufferFailure::MissingStorage)));
    drop(owner);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(image);
    assert_eq!(released.load(Ordering::SeqCst), 16);
    Ok(())
}

#[test]
fn overflow_preserves_bytes_cursor_and_first_failure() -> Result {
    let lease = lease(8, &Arc::default());
    let mut owner = Buffer::allocate(&lease, 8)?;
    owner.write(b"abcd")?;
    assert_eq!(owner.write(b"12345"), Err(BufferFailure::Full));
    assert_eq!(owner.position, 4);
    assert_eq!(owner.length(), 4);
    assert_eq!(&owner.bytes[..4], b"abcd");
    assert_eq!(owner.seek(-1, 0), Err(BufferFailure::Full));
    assert_eq!(owner.freeze(), Err(BufferFailure::Full));
    assert!(matches!(owner.take(), Err(BufferFailure::Full)));
    Ok(())
}

#[test]
fn seeks_use_written_extent_and_zeroed_holes_without_growing_capacity() -> Result {
    let lease = lease(16, &Arc::default());
    let mut owner = Buffer::allocate(&lease, 16)?;
    owner.write(b"ab")?;
    assert_eq!(owner.seek(3, 2)?, 5);
    owner.write(b"z")?;
    assert_eq!(owner.seek(-1, 1)?, 5);
    assert_eq!(owner.seek(0, 0)?, 0);
    let mut output = [99; 8];
    assert_eq!(owner.read(&mut output)?, 6);
    assert_eq!(output, [b'a', b'b', 0, 0, 0, b'z', 99, 99]);
    assert_eq!(owner.read(&mut output)?, 0);
    assert_eq!(owner.bytes.capacity(), 16);
    Ok(())
}

#[test]
fn negative_overflowing_and_foreign_seek_origins_invalidate_capture() -> Result {
    for (offset, origin) in [(-1, 0), (i64::MAX, 1), (1, 8), (17, 0)] {
        let lease = lease(16, &Arc::default());
        let mut owner = Buffer::allocate(&lease, 16)?;
        owner.write(b"a")?;
        assert_eq!(owner.seek(offset, origin), Err(BufferFailure::InvalidPosition));
        assert_eq!(owner.position, 1);
        assert_eq!(owner.freeze(), Err(BufferFailure::InvalidPosition));
    }
    Ok(())
}

#[test]
fn shrink_clears_retired_bytes_and_extension_cannot_restore_them() -> Result {
    let lease = lease(8, &Arc::default());
    let mut owner = Buffer::allocate(&lease, 8)?;
    owner.write(b"abcdefgh")?;
    owner.set_size(2)?;
    owner.set_size(8)?;
    owner.seek(0, 0)?;
    let mut output = [99; 8];
    assert_eq!(owner.read(&mut output)?, 8);
    assert_eq!(output, [b'a', b'b', 0, 0, 0, 0, 0, 0]);
    assert_eq!(owner.set_size(9), Err(BufferFailure::Full));
    assert_eq!(owner.length(), 8);
    Ok(())
}

#[test]
fn native_writes_after_real_completion_cannot_modify_or_publish_image() -> Result {
    let lease = lease(8, &Arc::default());
    let mut owner = Buffer::allocate(&lease, 8)?;
    owner.write(b"image")?;
    owner.freeze()?;
    assert_eq!(owner.write(b"late"), Err(BufferFailure::Frozen));
    assert!(matches!(owner.take(), Err(BufferFailure::Frozen)));
    assert_eq!(&owner.bytes[..5], b"image");
    Ok(())
}

#[test]
fn bytes_cannot_transfer_before_native_completion() -> Result {
    let lease = lease(8, &Arc::default());
    let mut owner = Buffer::allocate(&lease, 8)?;
    owner.write(b"image")?;
    assert!(matches!(owner.take(), Err(BufferFailure::InvalidPosition)));
    assert!(owner.claim.is_some());
    assert_eq!(owner.bytes.len(), 8);
    Ok(())
}

#[test]
fn insufficient_original_credit_rejects_before_allocating_image() -> Result {
    let lease = lease(IMAGE_BYTES as u64, &Arc::default());
    let held = lease.claim(1)?;
    assert!(Buffer::new(&lease).is_err());
    drop(held);
    let owner = Buffer::new(&lease)?;
    assert_eq!(owner.bytes.capacity(), IMAGE_BYTES);
    Ok(())
}
