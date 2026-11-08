// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve independent session FIFO admission across overload, callbacks and exact UTF-16 copying.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::execution::MemoryRelease;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn queue() -> Result<Rc<Queue>> {
    let memory = MemoryLease::new(24 * 1024 * 1024, Box::new(Release));
    Ok(Queue::new(Events::new(&memory, || {})?, &memory)?)
}
#[test]
fn independent_session_queue_accepts_full_chunks_reuses_storage_and_refuses_ninth() -> Result {
    let queue = queue()?;
    let pointers: Vec<_> =
        queue.state.borrow().slots.iter().map(|slot| slot.payload.as_ptr()).collect();
    for value in 0..CAPACITY + 1 {
        queue.record(
            |source| {
                source.push_str("http://127.0.0.1:8188/");
                Ok(())
            },
            |payload| {
                payload.extend(std::iter::repeat_n(char::from(b'A' + value as u8), 64 * 1024));
                Ok(())
            },
        );
    }
    assert_eq!(queue.state.borrow().count, CAPACITY);
    for value in 0..CAPACITY {
        let observed = queue.consume(|source, payload| {
            assert_eq!(source, "http://127.0.0.1:8188/");
            assert_eq!(payload.len(), 64 * 1024);
            assert!(payload.bytes().all(|byte| byte == b'A' + value as u8));
            Ok(())
        })?;
        assert_eq!(observed, Some(()));
    }
    assert!(!queue.pending());
    assert_eq!(
        queue.state.borrow().slots.iter().map(|slot| slot.payload.as_ptr()).collect::<Vec<_>>(),
        pointers
    );
    Ok(())
}
#[test]
fn native_callback_cannot_drain_and_close_revokes_all_waiting_input() -> Result {
    let queue = queue()?;
    queue.record(
        |_| Ok(()),
        |payload| {
            payload.push_str("queued");
            Ok(())
        },
    );
    {
        let _callback = queue.events.callback();
        assert!(queue.consume(|_, _| Ok(())).is_err());
    }
    assert!(queue.pending());
    queue.close();
    assert!(!queue.pending());
    queue.record(|_| Ok(()), |_| Ok(()));
    assert!(!queue.pending());
    Ok(())
}
#[test]
fn bounded_utf16_copy_keeps_unicode_and_refuses_malformed_or_oversized_before_growth() -> Result {
    let mut output = String::with_capacity(128);
    let original = output.as_ptr();
    let mut source: Vec<_> = "漢字🌸".encode_utf16().collect();
    source.push(0);
    copy(&mut output, 128, |index| source.get(index).copied().unwrap_or(0))?;
    assert_eq!(output, "漢字🌸");
    assert_eq!(output.as_ptr(), original);
    output.clear();
    assert!(copy(&mut output, 128, |index| if index == 0 { 0xD800 } else { 0 }).is_err());
    assert!(output.is_empty());
    assert!(copy(&mut output, 128, |_| u16::from(b'a')).is_err());
    assert!(output.is_empty());
    assert_eq!(output.as_ptr(), original);
    Ok(())
}
