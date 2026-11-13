// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify that retention gives memory back to the application budget cheapest-to-rebuild first.

use super::*;
use std::sync::Weak;

#[test]
fn retention_keeps_every_document_its_funded_allowance_holds() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    let mut first: Option<Weak<CpuSurface>> = None;
    for index in 0..18_u8 {
        fixture.show([255, index * 8, 0], &format!("output-{index}"))?;
        if index == 0 {
            first = fixture.session.document.front.as_ref().map(Arc::downgrade);
        }
    }
    // Bytes, not a count, bound retention: all seventeen outgoing documents stay parked.
    assert_eq!(fixture.session.retained.count(), 17);
    for index in 0..17 {
        assert!(fixture.session.retained.contains(&key(&format!("output-{index}"))));
    }
    assert!(first.ok_or("First detail backing absent")?.upgrade().is_some());
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn retention_demotes_before_it_evicts_beyond_its_byte_allowance() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([255, 0, 0], "a")?;
    fixture.show([0, 255, 0], "b")?;
    let one = fixture.session.retained.bytes();
    assert!(one > 0);
    let a_detail = fixture.session.retained_detail(&key("a")).ok_or("Parked detail absent")?;
    fixture.session.config.retained_document_bytes = 2 * one + one / 2;
    fixture.show([0, 0, 255], "c")?;
    fixture.show([255, 255, 0], "d")?;
    // The least recently shown document gave up its detail; nothing had to be evicted.
    assert_eq!(fixture.session.retained.count(), 3);
    assert!(fixture.session.retained_detail(&key("a")).is_none());
    assert!(fixture.session.retained_detail(&key("b")).is_some());
    assert!(fixture.session.retained_detail(&key("c")).is_some());
    assert!(fixture.session.retained.bytes() <= fixture.session.config.retained_document_bytes);
    fixture.settle_retirement()?;
    assert!(a_detail.upgrade().is_none(), "The demoted detail backing was not retired");
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn the_memory_budget_limit_demotes_then_evicts_and_can_widen_again() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    for (color, name) in
        [([255, 0, 0], "a"), ([0, 255, 0], "b"), ([0, 0, 255], "c"), ([255, 255, 0], "d")]
    {
        fixture.show(color, name)?;
    }
    let full = fixture.session.retained_bytes();
    assert_eq!(fixture.session.retained.count(), 3);
    // Half the parked bytes: the oldest documents demote first.
    fixture.session.set_retained_limit(full / 2);
    assert!(fixture.session.retained_bytes() <= full / 2);
    assert!(fixture.session.retained_detail(&key("a")).is_none());
    assert_eq!(fixture.session.retained.count(), 3);
    // Critical pressure keeps nothing parked; the current document is untouched.
    fixture.session.set_retained_limit(0);
    assert_eq!(fixture.session.retained.count(), 0);
    assert_eq!(fixture.session.retained_bytes(), 0);
    assert!(fixture.session.document.front.is_some());
    fixture.settle_retirement()?;
    // A wider limit lets retention grow again, but never past the funded allowance.
    fixture.session.set_retained_limit(u64::MAX);
    fixture.show([0, 255, 255], "e")?;
    assert!(fixture.session.retained.contains(&key("d")));
    assert!(fixture.session.retained_bytes() <= fixture.session.config.retained_document_bytes);
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn reselecting_a_demoted_document_renders_its_detail_without_acquiring_it()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::retaining()?;
    fixture.show([0, 255, 0], "b")?;
    fixture.show([255, 0, 0], "c")?;
    fixture.show([0, 0, 255], "d")?;
    // One byte under the parked total demotes only the least recently shown document.
    fixture.session.set_retained_limit(fixture.session.retained_bytes() - 1);
    assert!(fixture.session.retained_detail(&key("b")).is_none());
    assert!(fixture.session.retained_detail(&key("c")).is_some());
    assert!(fixture.session.retained.contains(&key("b")));
    assert!(fixture.session.reselect_retained(&key("b"))?);
    let selection = fixture.session.selection;
    let mut detail = None;
    for _ in 0..20 {
        let shown = fixture.next_shown(|session| {
            session.loading.is_none().then_some(()).ok_or("The demoted output was acquired again")
        })?;
        assert!(green(shown.pixel), "{:x}", shown.pixel);
        if shown.evidence.tier == BackingTier::Detail {
            detail = Some(shown);
            break;
        }
    }
    let detail = detail.ok_or("The demoted document never returned to full quality")?;
    assert_eq!(detail.evidence.source_revision, selection);
    fixture.settle()?;
    let _directory = fixture.finish()?;
    Ok(())
}
