// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production callback storage, byte bounds and ownership without native calls.

use super::*;
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::MemoryRelease;

struct Released(Arc<AtomicUsize>);
impl MemoryRelease for Released {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn lease(released: &Arc<AtomicUsize>) -> MemoryLease {
    MemoryLease::new(4 * 1024 * 1024, Box::new(Released(released.clone())))
}

#[test]
fn native_observation_storage_is_preclaimed_and_reused_in_fifo_order() -> Result<(), Box<dyn Error>>
{
    let released = Arc::new(AtomicUsize::new(0));
    let lease = lease(&released);
    let wakes = Rc::new(Cell::new(0));
    let counted = wakes.clone();
    let events = Events::new(&lease, move || counted.set(counted.get() + 1))?;
    let capacities: Vec<_> =
        events.storage.borrow().slots.iter().map(|s| s.payload.as_ptr()).collect();
    for index in 0..SLOTS {
        events.record(Kind::ContextDestroyed, |text| {
            text.push(char::from(b'A' + index as u8));
            Ok(())
        });
    }
    assert_eq!(events.remaining(), SLOTS);
    for index in 0..SLOTS {
        assert!(events.consume(|event| {
            assert_eq!(event.kind, Kind::ContextDestroyed);
            assert_eq!(event.payload.as_bytes(), &[b'A' + index as u8]);
            Ok(())
        })?);
    }
    assert!(!events.consume(|_| Ok(()))?);
    let storage = events.storage.borrow();
    assert_eq!(storage.slots.iter().map(|s| s.payload.as_ptr()).collect::<Vec<_>>(), capacities);
    assert!(storage._claim.bytes() <= lease.bytes());
    drop(storage);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(events);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(wakes.get(), SLOTS);
    Ok(())
}

#[test]
fn console_bursts_cannot_exhaust_lifecycle_slots_or_hide_setup_failures()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    events.record(Kind::Context, |text| {
        text.push_str("context");
        Ok(())
    });
    for _ in 0..1024 {
        events.record(Kind::Console, |text| {
            text.push_str(
                r#"{"type":"log","args":[{"type":"string","value":"Custom node loaded"}]}"#,
            );
            Ok(())
        });
    }
    let failure = r#"{"type":"error","executionContextId":7,"args":[{"type":"string","value":"Error calling extension 'SugarCubes.UI' method 'setup'"}]}"#;
    events.record(Kind::Console, |text| {
        text.push_str(failure);
        Ok(())
    });
    events.record(Kind::Navigation { succeeded: true }, |_| Ok(()));
    assert!(events.accepting());
    assert!(events.take_failure().is_none());
    assert_eq!(events.remaining(), 3);
    events.consume(|event| {
        assert_eq!(event.kind, Kind::Context);
        Ok(())
    })?;
    events.consume(|event| {
        assert_eq!(event.kind, Kind::Console);
        assert_eq!(event.payload, failure);
        Ok(())
    })?;
    events.consume(|event| {
        assert_eq!(event.kind, Kind::Navigation { succeeded: true });
        Ok(())
    })?;
    for _ in 0..SLOTS {
        events.record(Kind::Context, |_| Ok(()));
    }
    events.record(Kind::Console, |text| {
        text.push_str(r#"{"type":"debug","args":[]}"#);
        Ok(())
    });
    assert!(events.accepting());
    assert_eq!(events.remaining(), SLOTS);
    Ok(())
}

#[test]
fn untrusted_panel_bursts_and_oversized_payloads_cannot_close_lifecycle_admission()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    events.record(Kind::PanelBinding, |_| {
        Err(DesktopError::message("panel_fixture", "oversized native payload"))
    });
    for _ in 0..PANEL_SLOTS + 10 {
        events.record(Kind::PanelBinding, |text| {
            text.push_str("panel");
            Ok(())
        });
    }
    events.record(Kind::ContextsCleared, |_| Ok(()));
    assert!(events.accepting());
    assert!(events.take_failure().is_none());
    assert_eq!(events.remaining(), PANEL_SLOTS + 1);
    for _ in 0..PANEL_SLOTS {
        assert!(events.consume(|event| {
            assert_eq!(event.kind, Kind::PanelBinding);
            Ok(())
        })?);
    }
    assert!(events.consume(|event| {
        assert_eq!(event.kind, Kind::ContextsCleared);
        Ok(())
    })?);
    Ok(())
}
#[test]
fn receiver_overflow_stickily_closes_admission_without_losing_prior_facts()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    for _ in 0..=SLOTS {
        events.record(Kind::ContextsCleared, |_| Ok(()));
    }
    assert_eq!(events.remaining(), SLOTS);
    assert!(events.take_failure().is_some());
    let copied = Cell::new(false);
    events.record(Kind::Frame, |_| {
        copied.set(true);
        Ok(())
    });
    assert!(!copied.get());
    assert_eq!(events.remaining(), SLOTS);
    for _ in 0..SLOTS {
        assert!(events.consume(|event| {
            assert_eq!(event.kind, Kind::ContextsCleared);
            Ok(())
        })?);
    }
    assert!(!events.accepting.get());
    Ok(())
}

#[test]
fn active_callback_cannot_drain_and_failed_consumer_still_disposes_one_entry()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    events.record(Kind::Frame, |_| Ok(()));
    {
        let _callback = events.callback();
        assert!(events.consume(|_| Ok(())).is_err());
        assert_eq!(events.remaining(), 1);
    }
    assert!(
        events
            .consume(|_| Err(DesktopError::message("consumer", "original consumer cause")))
            .is_err()
    );
    assert_eq!(events.remaining(), 0);
    Ok(())
}

#[test]
fn independent_utf16_and_utf8_limits_reject_before_growth_or_partial_decode()
-> Result<(), Box<dyn Error>> {
    let mut output = String::new();
    output.try_reserve_exact(PAYLOAD_BYTES)?;
    let allocation = output.as_ptr();
    let exact = [0xd83d, 0xde42, 0];
    copy_utf16(&mut output, |index| exact[index])?;
    assert_eq!(output, "🙂");
    output.clear();
    assert!(copy_utf16(&mut output, |_| b'x' as u16).is_err());
    assert!(copy_utf16(&mut output, |index| if index == 1 { 0 } else { 0xd800 }).is_err());
    assert!(
        copy_utf16(&mut output, |index| if index == NATIVE_UNITS - 1 { 0 } else { 0x0800 })
            .is_err()
    );
    assert!(output.is_empty());
    assert_eq!(output.as_ptr(), allocation);
    assert_eq!(output.capacity(), PAYLOAD_BYTES);
    Ok(())
}

#[test]
fn native_utf16_admission_counts_the_terminator_without_growing_storage()
-> Result<(), Box<dyn Error>> {
    let mut output = String::new();
    output.try_reserve_exact(PAYLOAD_BYTES)?;
    let allocation = output.as_ptr();
    let mut exact = vec![u16::from(b'x'); NATIVE_UNITS - 1];
    exact.push(0);
    copy_utf16(&mut output, |index| exact[index])?;
    assert_eq!(output.len(), NATIVE_UNITS - 1);
    assert_eq!(output.as_ptr(), allocation);
    output.clear();
    exact.insert(NATIVE_UNITS - 1, u16::from(b'x'));
    assert!(copy_utf16(&mut output, |index| exact[index]).is_err());
    assert!(output.is_empty());
    assert_eq!(output.as_ptr(), allocation);
    Ok(())
}

#[test]
fn escaped_binding_envelopes_must_fit_before_current_context_ingress_can_decode()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{canvas_surface::CanvasSurfaceMessage, panel::PanelSession};

    let session = PanelSession::new("session-12345678".into())?;
    let mut request = serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":1, "type":"surface.select", "revision":1, "itemId":"result:one",
        "address":{"sessionId":"canvas:w", "viewId":"view:a", "windowId":"window:a",
            "attachmentId":"attachment:a", "generation":1},
        "file":{"filename":"\\".repeat(3800), "subfolder":"\\".repeat(3800), "type":"output"}
    });
    let payload = serde_json::to_string(&request)?;
    assert!(payload.len() < 16384);
    assert!(CanvasSurfaceMessage::decode(payload.as_bytes()).is_ok());
    let oversized = serde_json::json!({
        "name":crate::main_panel_ingress::BINDING_NAME,
        "payload":payload, "executionContextId":i64::MIN,
    })
    .to_string();
    assert!(oversized.len() < PAYLOAD_BYTES);
    let oversized: Vec<_> = oversized.encode_utf16().chain([0]).collect();
    assert!(oversized.len() > NATIVE_UNITS);
    let mut output = String::new();
    output.try_reserve_exact(PAYLOAD_BYTES)?;
    let allocation = output.as_ptr();
    assert!(copy_utf16(&mut output, |index| oversized[index]).is_err());
    assert!(output.is_empty());
    request["file"]["filename"] = serde_json::json!("résumé🙂.png");
    request["file"]["subfolder"] = serde_json::json!("folder\\nested");
    let admitted = serde_json::json!({
        "name":crate::main_panel_ingress::BINDING_NAME,
        "payload":serde_json::to_string(&request)?, "executionContextId":i64::MIN,
    })
    .to_string();
    let admitted: Vec<_> = admitted.encode_utf16().chain([0]).collect();
    copy_utf16(&mut output, |index| admitted[index])?;
    assert!(matches!(
        crate::main_panel_ingress::decode(&output, Some(i64::MIN), &session),
        Some(crate::main_panel_ingress::ClientMessage::Surface(CanvasSurfaceMessage::Request(_)))
    ));
    assert!(crate::main_panel_ingress::decode(&output, Some(17), &session).is_none());
    assert_eq!(output.as_ptr(), allocation);
    Ok(())
}

#[test]
fn existing_claim_exhaustion_prevents_receiver_retention() -> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let lease = lease(&released);
    let claim = lease.claim(lease.bytes())?;
    assert!(Events::new(&lease, || {}).is_err());
    drop(claim);
    let events = Events::new(&lease, || {})?;
    events.close_admission();
    assert!(!events.accepting.get());
    Ok(())
}

/// Fixed command metadata must not leak across FIFO slot reuse or callback admission close.
#[test]
fn completion_metadata_is_immutable_and_cleared_before_slot_reuse() -> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    events.record_command(Command::Runtime, |text| {
        text.push_str("{}");
        Ok(())
    });
    events.record_command(Command::Page, |text| {
        text.push_str("{\"ok\":true}");
        Ok(())
    });
    for (command, payload) in [(Command::Runtime, "{}"), (Command::Page, "{\"ok\":true}")] {
        assert!(events.consume(|observation| {
            assert_eq!(observation.kind, Kind::Command);
            assert_eq!(observation.command, Some(command));
            assert_eq!(observation.payload, payload);
            Ok(())
        })?);
    }
    for _ in 0..SLOTS {
        events.record(Kind::ContextsCleared, |_| Ok(()));
        assert!(events.consume(|observation| {
            assert_eq!(observation.kind, Kind::ContextsCleared);
            assert_eq!(observation.command, None);
            assert_eq!(observation.payload, "");
            Ok(())
        })?);
    }
    events.close_admission();
    let copied = Cell::new(false);
    events.record_command(Command::Runtime, |_| {
        copied.set(true);
        Ok(())
    });
    assert!(!copied.get());
    assert_eq!(events.remaining(), 0);
    Ok(())
}

#[test]
fn native_process_failure_is_observable_before_earlier_context_teardown_without_drain()
-> Result<(), Box<dyn Error>> {
    let released = Arc::new(AtomicUsize::new(0));
    let events = Events::new(&lease(&released), || {})?;
    events.record(Kind::ContextDestroyed, |_| Ok(()));
    events.record(Kind::ProcessFailed { kind: 1 }, |_| Ok(()));
    assert_eq!(events.process_failed(), Some(1));
    assert_eq!(events.remaining(), 2);
    assert!(events.consume(|observation| {
        assert_eq!(observation.kind, Kind::ContextDestroyed);
        Ok(())
    })?);
    assert_eq!(events.process_failed(), Some(1));
    events.close_admission();
    events.record(Kind::ProcessFailed { kind: 2 }, |_| Ok(()));
    assert_eq!(events.process_failed(), Some(1));
    Ok(())
}
