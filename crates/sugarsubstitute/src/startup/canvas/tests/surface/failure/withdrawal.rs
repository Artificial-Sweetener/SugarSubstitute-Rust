// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cancel a selection at each worker stage and require one exact terminal and full retirement.

use super::*;

/// Withdraw a blue selection at `stage`, then require the next selection to present normally.
fn withdraw(stage: Stage) -> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let before = host.committed().len();
        let gate = select_at(host, Address::ORIGINAL, 5, "tall.png", stage, true)?;
        let prepared = prepared_frame(&host.fixture.canvas)
            .filter(|frame| frame.content.pending())
            .map(|frame| frame.content.target());
        assert_eq!(prepared.is_some(), stage == Stage::Present);
        assert_eq!(host.cancel(5)?.as_deref(), Some("cancelled"));
        if let Some(target) = prepared {
            assert!(host.fixture.canvas.presented(target).is_err());
        }
        // A duplicate cancellation only consumes its own receipt.
        assert_eq!(host.cancel(5)?, None);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Red));
        let committed = host.committed();
        assert!(!committed[before..].contains(&Shade::Blue), "{committed:?}");
        if let Some(gate) = &gate {
            assert!(gate.cancelled());
            assert_eq!(gate.retirements(), usize::from(stage == Stage::Acquire));
        }
        idle(host)?;
        host.send(1, 6, "surface.select", select("green.png"))?;
        host.expect(6, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        idle(host)
    })
}

#[test]
fn cancellation_at_every_stage_has_one_terminal_and_retires_all_work() -> Result<(), Box<dyn Error>>
{
    all([Stage::Acquire, Stage::Load, Stage::Render, Stage::Present], withdraw)
}

#[test]
fn cancellation_after_presentation_only_consumes_its_receipt() -> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        host.send(1, 5, "surface.select", select("tall.png"))?;
        host.expect(5, "applied")?;
        assert_eq!(host.cancel(5)?, None);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        idle(host)
    })
}

#[test]
fn cancelling_a_staged_preparation_restores_the_committed_attachment() -> Result<(), Box<dyn Error>>
{
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let staged = Address { workflow: "workflow", attachment: 2 };
        host.send_to(staged, 5, "surface.prepare", json!({}))?;
        // Preparation may complete synchronously; a ready candidate is then released instead.
        match host.cancel(5)?.as_deref() {
            Some("cancelled") => {}
            Some("ready") => {
                host.send_to(staged, 6, "surface.release", json!({}))?;
                host.expect(6, "applied")?;
            }
            other => return Err(format!("Preparation withdrawal replied {other:?}").into()),
        }
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Red));
        idle(host)
    })
}
