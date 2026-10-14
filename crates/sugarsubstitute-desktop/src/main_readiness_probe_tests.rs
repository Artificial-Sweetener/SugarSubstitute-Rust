// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Repeated UI polling preserves genuine issued native-operation and receipt custody.
use super::*;

#[test]
fn repeated_polls_keep_one_issued_evaluation_through_its_exact_native_barrier() -> Result {
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    let next = owner.next_operation;
    let mut issued = vec![target];
    for _ in 0..32 {
        if let Some(second) = owner.poll_probe()? {
            issued.push(second);
        }
    }
    assert_eq!(issued, [target]);
    assert_eq!(owner.next_operation, next);
    assert_eq!(owner.pending.as_ref().map(|pending| pending.target), Some(target));
    assert_eq!(owner.issue_probe(), Err(ReadinessFault::ProbeBusy));
    assert!(owner.dom_result(target, Some(positive(target)?))?);
    for _ in 0..32 {
        assert!(owner.poll_probe()?.is_none());
        let pending = owner.pending.as_ref().ok_or("Actual issued barrier lost")?;
        assert_eq!(pending.target, target);
        let receipt = pending.receipt.as_ref().ok_or("Original positive DOM receipt lost")?;
        assert_eq!(receipt.launch(), target.identity.launch);
        assert_eq!(receipt.navigation(), target.identity.navigation);
        assert_eq!(owner.next_operation, next);
        assert!(!owner.ready());
    }
    let receipt = owner.barrier_complete(target)?;
    assert_eq!(receipt.launch(), target.identity.launch);
    assert!(owner.ready());
    assert!(owner.poll_probe()?.is_none());
    assert_eq!(owner.next_operation, next);
    assert_eq!(issued.len(), 1);
    Ok(())
}

#[test]
fn incomplete_callback_alone_allows_next_operation_and_foreign_callbacks_keep_current_custody()
-> Result {
    let mut owner = owner()?;
    let first = loaded(&mut owner)?;
    let mut foreign = first;
    foreign.identity.launch = LaunchId::new(2)?;
    assert_eq!(owner.dom_result(foreign, None), Err(ReadinessFault::StaleProbe));
    assert!(owner.poll_probe()?.is_none());
    assert_eq!(owner.pending.as_ref().map(|pending| pending.target), Some(first));
    assert!(!owner.dom_result(first, None)?);
    let second = owner.poll_probe()?.ok_or("Next actual evaluation missing")?;
    assert_eq!(second.operation, first.operation + 1);
    assert_eq!(second.identity, first.identity);
    assert_eq!(second.context, first.context);
    assert_eq!(second.frame, first.frame);
    assert_eq!(second.loader, first.loader);
    assert_eq!(owner.dom_result(first, Some(positive(first)?)), Err(ReadinessFault::StaleProbe));
    assert_eq!(owner.pending.as_ref().map(|pending| pending.target), Some(second));
    assert!(owner.poll_probe()?.is_none());
    assert!(owner.dom_result(second, Some(positive(second)?))?);
    assert_eq!(owner.barrier_complete(first), Err(ReadinessFault::StaleProbe));
    assert!(owner.poll_probe()?.is_none());
    owner.barrier_complete(second)?;
    assert!(owner.ready());
    Ok(())
}

#[test]
fn repeated_polling_never_coalesces_away_context_loss_or_malformed_duplicate_receipts() -> Result {
    let mut context = owner()?;
    let target = loaded(&mut context)?;
    assert!(context.poll_probe()?.is_none());
    assert_eq!(context.context_destroyed(target.context), Err(ReadinessFault::ContextLost));
    assert_eq!(context.poll_probe(), Err(ReadinessFault::ContextLost));
    assert!(context.pending.is_none());
    assert_eq!(context.next_operation, target.operation + 1);
    assert_eq!(context.barrier_complete(target), Err(ReadinessFault::ContextLost));
    assert!(!context.ready());

    let mut malformed = owner()?;
    let target = loaded(&mut malformed)?;
    assert!(malformed.dom_result(target, Some(positive(target)?))?);
    assert!(malformed.poll_probe()?.is_none());
    assert_eq!(
        malformed.dom_result(target, Some(positive(target)?)),
        Err(ReadinessFault::MalformedObservation)
    );
    assert_eq!(malformed.poll_probe(), Err(ReadinessFault::MalformedObservation));
    assert_eq!(malformed.barrier_complete(target), Err(ReadinessFault::MalformedObservation));
    assert!(malformed.pending.is_none());
    assert_eq!(malformed.next_operation, target.operation + 1);
    assert!(!malformed.ready());
    Ok(())
}

#[test]
fn frontend_timeout_snapshot_is_passive_and_tracks_real_probe_barrier_and_fault_custody() -> Result
{
    use crate::main_readiness::snapshot::Wait;
    let mut value = owner()?;
    assert_eq!(value.wait_snapshot().wait, Wait::Setup);
    value.navigation_started()?;
    assert_eq!(value.wait_snapshot().wait, Wait::Navigation);
    value.navigation_completed(true)?;
    assert_eq!(value.wait_snapshot().wait, Wait::Root);
    value.frame("root", "loader", None, "http://127.0.0.1:8188/")?;
    assert_eq!(value.wait_snapshot().wait, Wait::Context);
    value.context(7, "root", "http://127.0.0.1:8188", true)?;
    assert_eq!(value.wait_snapshot().wait, Wait::Document);
    let target = value.poll_probe()?.ok_or("Actual target absent")?;
    let sequence = value.next_operation;
    for _ in 0..32 {
        assert_eq!(value.wait_snapshot().wait, Wait::Probe);
        assert_eq!(value.wait_snapshot().operation, Some(target.operation));
    }
    assert_eq!(value.next_operation, sequence);
    value.dom_result(target, Some(positive(target)?))?;
    assert_eq!(value.wait_snapshot().wait, Wait::Barrier);
    assert_eq!(value.wait_snapshot().operation, Some(target.operation));
    let mut stale = target;
    stale.identity.navigation = Generation::new(2)?;
    assert_eq!(value.barrier_complete(stale), Err(ReadinessFault::StaleProbe));
    assert_eq!(value.wait_snapshot().wait, Wait::Barrier);
    value.barrier_complete(target)?;
    assert_eq!(value.wait_snapshot().wait, Wait::Ready);
    assert_eq!(value.wait_snapshot().operation, None);
    value.invalidate(ReadinessFault::ContextLost);
    assert_eq!(value.wait_snapshot().wait, Wait::Fault);
    assert_eq!(value.wait_snapshot().operation, None);
    Ok(())
}
