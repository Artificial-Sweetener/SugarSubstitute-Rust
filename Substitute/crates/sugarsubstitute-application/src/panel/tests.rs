// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise panel handshake decisions without a browser or native transport.

use super::*;
use sugarsubstitute_contracts::panel::{PanelPalette, PanelRejection, PanelStyle};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn moment(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}

fn fixture() -> Result<(PanelHostSession, PanelSession, PanelPresentation)> {
    let session = PanelSession::new("session-12345678".into())?;
    let presentation =
        PanelPresentation::new(false, PanelStyle::Comfy, Some(PanelPalette::DarkGray), None, 0)?;
    let host = PanelHostSession::new(session.clone(), presentation.clone(), moment(0))?;
    Ok((host, session, presentation))
}

#[test]
fn current_session_handshake_preserves_presentation_and_connection_state() -> Result<()> {
    let (mut host, session, presentation) = fixture()?;
    let ready = host.receive(PanelMessage::Ready(session.clone()), moment(1))?;
    assert_eq!(
        ready.send,
        Some(PanelMessage::Configure {
            session: session.clone(),
            presentation: presentation.clone()
        })
    );
    assert_eq!(ready.state, None);
    let applied = host.receive(PanelMessage::Applied(session.clone()), moment(2))?;
    assert_eq!(applied.send, Some(PanelMessage::Ping(session.clone())));
    assert_eq!(host.next_deadline()?, Some(moment(7)));
    let pong =
        host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(3))?;
    assert_eq!(pong.state, Some(PanelState::Connected));
    assert_eq!(host.state(), PanelState::Connected);
    assert_eq!(
        host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(4))?
            .state,
        None
    );
    assert_eq!(host.update_presentation(presentation).send, None);
    let changed =
        PanelPresentation::new(false, PanelStyle::Fluent, Some(PanelPalette::Black), None, 0)?;
    assert_eq!(
        host.update_presentation(changed.clone()).send,
        Some(PanelMessage::Configure { session, presentation: changed })
    );
    Ok(())
}

#[test]
fn startup_and_heartbeat_deadlines_are_fixed_and_recover_after_a_fresh_pong() -> Result<()> {
    let (mut waiting, _, _) = fixture()?;
    assert_eq!(waiting.next_deadline()?, Some(moment(30)));
    assert_eq!(waiting.tick(moment(29))?.state, None);
    assert_eq!(waiting.tick(moment(30))?.state, Some(PanelState::Failed));
    assert_eq!(waiting.next_deadline()?, None);

    let (mut host, session, _) = fixture()?;
    host.receive(PanelMessage::Ready(session.clone()), moment(0))?;
    host.receive(PanelMessage::Applied(session.clone()), moment(1))?;
    host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(2))?;
    let overdue = host.tick(moment(17))?;
    assert_eq!(overdue.state, Some(PanelState::Disconnected));
    assert_eq!(overdue.send, Some(PanelMessage::Ping(session.clone())));
    assert_eq!(host.next_deadline()?, Some(moment(22)));
    assert_eq!(host.retry_connection(moment(17))?.send, None);
    assert_eq!(
        host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(18))?
            .state,
        Some(PanelState::Connected)
    );
    assert_eq!(host.retry_connection(moment(19))?.send, Some(PanelMessage::Ping(session)));
    Ok(())
}

#[test]
fn expiration_does_not_consume_a_due_heartbeat() -> Result<()> {
    let (mut host, session, _) = fixture()?;
    host.receive(PanelMessage::Ready(session.clone()), moment(0))?;
    host.receive(PanelMessage::Applied(session.clone()), moment(1))?;
    host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(2))?;
    assert_eq!(host.next_expiration()?, Some(moment(17)));
    assert_eq!(host.expire(moment(17))?.state, Some(PanelState::Disconnected));
    assert_eq!(host.next_expiration()?, None);
    assert_eq!(host.next_deadline()?, Some(moment(6)));
    assert_eq!(host.tick(moment(17))?.send, Some(PanelMessage::Ping(session)));
    Ok(())
}

#[test]
fn foreign_and_out_of_phase_messages_cannot_configure_or_revive_a_session() -> Result<()> {
    let (mut host, session, _) = fixture()?;
    let foreign = PanelSession::new("foreign-12345678".into())?;
    assert_eq!(host.receive(PanelMessage::Ready(foreign), moment(0))?, PanelEffects::default());
    assert_eq!(
        host.receive(PanelMessage::Applied(session.clone()), moment(0))?,
        PanelEffects::default()
    );
    assert_eq!(
        host.receive(PanelMessage::Pong { session: session.clone(), connected: true }, moment(0))?,
        PanelEffects::default()
    );
    host.receive(PanelMessage::Ready(session.clone()), moment(0))?;
    let rejected = PanelMessage::Rejected {
        session: session.clone(),
        reason: PanelRejection::PresentationFailed,
    };
    assert_eq!(host.receive(rejected, moment(1))?.state, Some(PanelState::Failed));
    assert_eq!(host.receive(PanelMessage::Ready(session), moment(2))?, PanelEffects::default());
    Ok(())
}

#[test]
fn backward_time_and_unrepresentable_deadlines_never_mutate_the_session() -> Result<()> {
    let (mut host, session, presentation) = fixture()?;
    host.receive(PanelMessage::Ready(session.clone()), moment(2))?;
    assert_eq!(host.tick(moment(1)).err(), Some(PanelClockError::Regressed));
    assert_eq!(host.state(), PanelState::Loading);
    assert_eq!(host.next_deadline()?, Some(moment(30)));
    assert_eq!(
        PanelHostSession::new(session, presentation, Moment::from_nanos(u64::MAX)).err(),
        Some(PanelClockError::DeadlineOverflow)
    );
    Ok(())
}
