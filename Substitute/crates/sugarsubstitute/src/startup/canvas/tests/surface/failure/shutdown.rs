// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close the application canvas while selection work is in flight at each worker stage.

use super::*;

/// Close while a blue selection is held at `stage`; finishing proves settlement, owner join
/// and the return of every request credit under the original cutoff.
fn close_at(stage: Stage) -> Result<(), Box<dyn Error>> {
    let mut host = Host::new(0)?;
    let gate = commit_red(&mut host)
        .and_then(|()| select_at(&mut host, Address::ORIGINAL, 5, "tall.png", stage, true));
    let finished = host.finish();
    let gate = gate?;
    finished?;
    if let Some(gate) = gate {
        assert!(gate.cancelled() || stage == Stage::Load);
        assert_eq!(gate.retirements(), usize::from(stage == Stage::Acquire));
    }
    Ok(())
}

#[test]
fn closing_during_acquisition_load_render_or_presentation_retires_all_work()
-> Result<(), Box<dyn Error>> {
    all([Stage::Acquire, Stage::Load, Stage::Render, Stage::Present], close_at)
}

#[test]
fn a_hung_acquisition_keeps_close_unsettled_past_its_cutoff_until_it_returns()
-> Result<(), Box<dyn Error>> {
    let mut host = Host::new(0)?;
    let hung = host.gate("tall.png", false);
    let held = (|| -> Result<(), Box<dyn Error>> {
        commit_red(&mut host)?;
        host.send(1, 5, "surface.select", select("tall.png"))?;
        host.pump_until("acquisition", |_| hung.entered())?;
        let cutoff = host.fixture.clock.now().checked_add(Duration::from_secs(5))?;
        host.fixture.canvas.close_until(cutoff)?;
        assert!(hung.cancelled());
        host.fixture.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        assert!(!host.fixture.canvas.settled()?);
        host.fixture.advance(10_000);
        host.fixture.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        // Expiry is reported once; it never detaches the blocked worker or claims settlement.
        let expired = host.fixture.canvas.settled().err().ok_or("Cutoff expiry went unreported")?;
        assert_eq!(expired.to_string(), "Canvas shutdown exceeded its original deadline");
        assert!(!host.fixture.canvas.settled()?);
        assert!(!hung.returned());
        assert_eq!(host.fixture.execution()?, Execution::Running);
        Ok(())
    })();
    // The hung call finally returns after the cutoff. Its late result cannot be admitted for
    // retirement past the original budget, so the canvas never reports a false settlement; the
    // coordinator's job termination contains the process from here.
    hung.release();
    held?;
    let late = host.finish().err().ok_or("A post-cutoff close claimed settlement")?;
    assert!(format!("{late:?}").contains("DeadlineExpired"), "{late:?}");
    Ok(())
}
