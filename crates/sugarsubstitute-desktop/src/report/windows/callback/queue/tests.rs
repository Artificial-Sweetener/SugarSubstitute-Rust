// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise bounded ordered input and exact captured bindings at the actual queue owner.
use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
fn input(revision: u64) -> Result<Input, Box<dyn Error>> {
    let key = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(Input {
        binding: ReportBinding::new(
            IncidentKey::new(IncidentSubject::Supervisor(key)),
            IncidentRevision::new(revision)?,
            ReportSessionId::new(1)?,
        ),
        intent: super::super::Intent::Action(ReportAction::CopyText),
    })
}
#[test]
fn report_input_queue_bounds_order_and_coalesces_wakes_without_rebinding()
-> Result<(), Box<dyn Error>> {
    let queue = Queue::new();
    for index in 1..=32 {
        assert_eq!(queue.push(input(index)?)?, index == 1);
    }
    assert_eq!(
        queue.push(input(33)?).err().ok_or("unbounded input accepted")?.operation(),
        "report_input_queue_full"
    );
    let drained = queue.drain()?;
    for (index, value) in drained.into_iter().enumerate() {
        assert_eq!(value.ok_or("lost input")?.binding, input(index as u64 + 1)?.binding);
    }
    assert!(queue.drain()?.iter().all(Option::is_none));
    assert!(queue.push(input(34)?)?);
    Ok(())
}
#[test]
fn report_input_recursive_push_and_drain_preserve_original_slots_and_wake()
-> Result<(), Box<dyn Error>> {
    let queue = Queue::new();
    assert!(queue.push(input(1)?)?);
    let borrow = queue.slots.borrow_mut();
    assert_eq!(
        queue.push(input(2)?).err().ok_or("recursive push accepted")?.operation(),
        "report_input_queue_recursive"
    );
    assert_eq!(
        queue.drain().err().ok_or("recursive drain accepted")?.operation(),
        "report_input_drain_recursive"
    );
    assert!(queue.wake.get());
    drop(borrow);
    let drained = queue.drain()?;
    assert_eq!(drained[0].ok_or("lost original")?.binding, input(1)?.binding);
    assert!(drained.iter().skip(1).all(Option::is_none));
    Ok(())
}
