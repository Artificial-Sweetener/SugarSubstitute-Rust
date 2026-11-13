// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep presentation mutation separate from frontend readiness and exact native document custody.
use super::*;
use sugarsubstitute_execution::qualification_time::anchor;

#[test]
fn frame_configuration_requires_fresh_readonly_sample_and_original_completion_barrier() -> Result {
    let now = anchor();
    let mut owner = owner()?;
    let target = loaded(&mut owner)?;
    owner.begin_frame_configuration(target)?;
    assert!(owner.poll_probe(now)?.is_none());
    assert!(!owner.ready());
    owner.complete_frame_configuration(target, now)?;
    assert!(!owner.ready());
    let next = owner.poll_probe(now + Duration::from_millis(100))?.ok_or("Fresh sample missing")?;
    assert_eq!(next.operation, target.operation + 1);
    assert_eq!(
        (next.identity, next.context, next.frame, next.loader),
        (target.identity, target.context, target.frame, target.loader)
    );
    owner.dom_result(next, Some(positive(next)?), now)?;
    assert!(!owner.ready());
    owner.barrier_complete(next)?;
    assert!(owner.ready());
    Ok(())
}

#[test]
fn frame_configuration_rejects_foreign_duplicate_and_readonly_completion_substitution() -> Result {
    let now = anchor();
    let mut foreign = owner()?;
    let target = loaded(&mut foreign)?;
    foreign.begin_frame_configuration(target)?;
    let mut old = target;
    old.operation += 1;
    assert_eq!(foreign.complete_frame_configuration(old, now), Err(ReadinessFault::StaleProbe));
    assert!(foreign.poll_probe(now)?.is_none());
    foreign.complete_frame_configuration(target, now)?;
    assert_eq!(foreign.complete_frame_configuration(target, now), Err(ReadinessFault::StaleProbe));
    let mut duplicate = owner()?;
    let target = loaded(&mut duplicate)?;
    duplicate.begin_frame_configuration(target)?;
    assert_eq!(
        duplicate.begin_frame_configuration(target),
        Err(ReadinessFault::MalformedObservation)
    );
    let mut substituted = owner()?;
    let target = loaded(&mut substituted)?;
    substituted.begin_frame_configuration(target)?;
    assert_eq!(
        substituted.dom_result(target, Some(positive(target)?), now),
        Err(ReadinessFault::MalformedObservation)
    );
    assert!(!substituted.ready());
    Ok(())
}

#[test]
fn context_loss_and_cancellation_veto_pending_frame_configuration() -> Result {
    let now = anchor();
    let mut lost = owner()?;
    let target = loaded(&mut lost)?;
    lost.begin_frame_configuration(target)?;
    assert_eq!(lost.context_destroyed(target.context), Err(ReadinessFault::ContextLost));
    assert_eq!(lost.complete_frame_configuration(target, now), Err(ReadinessFault::ContextLost));
    let mut cancelled = owner()?;
    let target = loaded(&mut cancelled)?;
    cancelled.begin_frame_configuration(target)?;
    cancelled.invalidate(ReadinessFault::Cancelled);
    assert_eq!(cancelled.complete_frame_configuration(target, now), Err(ReadinessFault::Cancelled));
    assert!(!lost.ready() && !cancelled.ready());
    Ok(())
}
