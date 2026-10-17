// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain backend cancellation outcomes arriving on either side of the final desktop ACK.
use super::*;

/// A physically written final ACK ends delivery, while actual resource retirement stays required.
#[test]
fn late_cancelled_acquire_does_not_strand_closed_desktop_or_claim_a_fictitious_write() -> Result<()>
{
    for cancelled_before_ack in [false, true] {
        let mut parent = actor()?;
        parent.connected = true;
        intent(&mut parent, CompletionIntent::Normal)?;
        parent.registered = true;
        let original = parent.cleanup;
        let (acquire, _) = parent.requests.admit(header(1)?, b"original pending Acquire")?;
        let run = parent.run;
        parent
            .exit
            .as_mut()
            .ok_or("Exit owner absent")?
            .observe(DesktopObservation::Completed { run, shutdown: ShutdownId::new(1)? })
            .map_err(|rejected| rejected.reason)?;
        let (completed, _) = parent.requests.admit(header(2)?, b"final ExitCompleted")?;
        parent.requests.get_mut(completed)?.reply = Some(Reply::Acknowledged);
        parent.close_after_reply = Some(completed);
        if cancelled_before_ack {
            parent.requests.get_mut(acquire)?.reply = Some(Reply::Cancelled);
        }
        assert!(!parent.normal_cleanup_ready());
        // External native delivery facts cross the same receipt owner as the existing close test.
        let mut receipt = super::super::super::write_receipt::WriteReceipt::new(7, 104);
        receipt.issue(7)?;
        assert!(!parent.normal_cleanup_ready());
        receipt.complete(7, 104)?;
        parent.requests.get_mut(completed)?.sent = true;
        parent.closed = true;
        parent.requests.get_mut(acquire)?.reply = Some(Reply::Cancelled);
        assert_eq!(parent.requests.next_reply(), Some(acquire));
        assert!(!parent.requests.get(acquire)?.sent);
        assert!(parent.normal_cleanup_ready(), "an undeliverable older reply vetoed cleanup");
        assert!(!parent.native_retired());
        assert!(!parent.settled());
        assert_eq!(parent.cleanup, original);
        parent.requests.get_mut(completed)?.sent = false;
        assert!(!parent.normal_cleanup_ready(), "an unwritten ACK authorized cleanup");
        parent.requests.get_mut(completed)?.sent = true;
        parent.close_after_reply = None;
        assert!(!parent.normal_cleanup_ready(), "unattributed closure authorized cleanup");
    }
    Ok(())
}
