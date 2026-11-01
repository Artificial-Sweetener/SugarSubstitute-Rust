// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Release a superseded selection's completions after its successor in every reachable order.

use super::*;

/// Which attachment receives the two selections.
#[derive(Clone, Copy, Debug)]
enum Target {
    /// The committed visible attachment, presenting each selection directly.
    Committed,
    /// A hidden staged candidate, revealed and committed after the second selection.
    Staged,
}

#[derive(Clone, Copy, Debug)]
struct Order {
    target: Target,
    /// Progress of the first selection when the page withdraws it.
    stage: Stage,
    /// Whether a gated first acquisition stops on cancellation or returns after its successor.
    cooperative: bool,
}

/// Select blue, withdraw it at `stage`, select green, then complete the superseded work late.
fn supersede(order: Order) -> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let before = host.committed().len();
        let (address, first) = match order.target {
            Target::Committed => (Address::ORIGINAL, 5),
            Target::Staged => {
                let staged = Address { workflow: "workflow", attachment: 2 };
                host.send_to(staged, 5, "surface.prepare", json!({}))?;
                host.expect(5, "ready")?;
                host.send_to(staged, 6, "surface.geometry", geometry(80, 40, false))?;
                host.expect(6, "applied")?;
                (staged, 7)
            }
        };
        let gate = select_at(host, address, first, "tall.png", order.stage, order.cooperative)?;
        let withdrawn = host.cancel(first)?;
        match order.target {
            Target::Committed => assert_eq!(withdrawn.as_deref(), Some("cancelled")),
            // A hidden candidate replies once its pixels are prepared, before any commit.
            Target::Staged => {
                assert!(matches!(withdrawn.as_deref(), Some("cancelled" | "applied")))
            }
        }
        host.send_to(address, first + 1, "surface.select", select("green.png"))?;
        if let Some(gate) = &gate {
            assert!(gate.cancelled());
            // A non-cooperative acquisition now returns its original after the newer selection.
            gate.release();
        }
        answers(&host.expect(first + 1, "applied")?, address, first + 1);
        if matches!(order.target, Target::Staged) {
            host.send_to(address, first + 2, "surface.geometry", geometry(80, 40, true))?;
            host.expect(first + 2, "applied")?;
            host.send_to(address, first + 3, "surface.commit", json!({}))?;
            host.expect(first + 3, "applied")?;
        }
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        let committed = host.committed();
        assert!(!committed[before..].contains(&Shade::Blue), "{committed:?}");
        if let Some(gate) = gate
            && order.cooperative
            && order.stage == Stage::Acquire
        {
            assert_eq!(gate.retirements(), 1);
        }
        idle(host)
    })
}

#[test]
fn superseded_selections_never_present_in_any_completion_order() -> Result<(), Box<dyn Error>> {
    let mut orders = Vec::new();
    for target in [Target::Committed, Target::Staged] {
        for stage in [Stage::Acquire, Stage::Load, Stage::Render, Stage::Present] {
            // Uncommitted candidates never request native presentation.
            if matches!(target, Target::Staged) && stage == Stage::Present {
                continue;
            }
            let gated = matches!(stage, Stage::Acquire | Stage::Load);
            for cooperative in if gated { [true, false].as_slice() } else { [true].as_slice() } {
                orders.push(Order { target, stage, cooperative: *cooperative });
            }
        }
    }
    all(orders, supersede)
}

#[test]
fn a_selection_refused_while_another_is_pending_leaves_the_admitted_one_to_present()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let tall = host.gate("tall.png", false);
        host.send(1, 5, "surface.select", select("tall.png"))?;
        host.pump_until("acquisition", |_| tall.entered())?;
        host.send(1, 6, "surface.select", select("green.png"))?;
        answers(&host.expect(6, "unavailable")?, Address::ORIGINAL, 6);
        host.send(1, 7, "surface.commit", json!({}))?;
        answers(&host.expect(7, "unavailable")?, Address::ORIGINAL, 7);
        assert_eq!(host.shown(), Some(Shade::Red));
        tall.release();
        answers(&host.expect(5, "applied")?, Address::ORIGINAL, 5);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        host.send(1, 8, "surface.select", select("green.png"))?;
        host.expect(8, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        idle(host)
    })
}
