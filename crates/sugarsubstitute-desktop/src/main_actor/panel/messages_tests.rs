// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise host message back-pressure, ordering and navigation retirement without a browser.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::{PanelPalette, PanelPresentation, PanelSession, PanelStyle},
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

const CONTEXT: i64 = 17;

fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

fn ping(identity: MainIdentity) -> Result<PanelMessage, Box<dyn Error>> {
    Ok(PanelMessage::Ping(PanelSession::for_main(identity)?))
}

fn configure(identity: MainIdentity, style: PanelStyle) -> Result<PanelMessage, Box<dyn Error>> {
    Ok(PanelMessage::Configure {
        session: PanelSession::for_main(identity)?,
        presentation: PanelPresentation::new(false, style, PanelPalette::DarkGray, None, 0)?,
    })
}

fn message(
    identity: MainIdentity,
    value: PanelMessage,
    context: i64,
    lease: &MemoryLease,
) -> Result<HostMessage, Box<dyn Error>> {
    let command = PanelCommand::new(identity, value, lease.claim(PanelCommand::STORAGE_BYTES)?)?;
    Ok(HostMessage::new(command, context)?)
}

fn expected(value: &PanelMessage, context: i64) -> Result<String, Box<dyn Error>> {
    Ok(delivery::encode(context, &value.encode()?)?)
}

/// The browser's four-slot command table with the same tiered admission and fatal exhaustion.
#[derive(Default)]
struct StalledBrowser {
    pending: usize,
    submitted: Vec<(i64, String)>,
}
impl StalledBrowser {
    fn admits_panel_delivery(&self) -> bool {
        self.pending < 3
    }
    fn call(&mut self, message: &HostMessage) -> Result<(), DesktopError> {
        if self.pending == 4 {
            return Err(DesktopError::message("submit_main_command", "command capacity exhausted"));
        }
        self.pending += 1;
        self.submitted.push((message.context(), message.parameters().to_owned()));
        Ok(())
    }
}

#[test]
fn messages_beyond_free_capacity_are_retained_then_submitted_in_order_as_slots_free()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(2 * PanelCommand::STORAGE_BYTES, Box::new(Release));
    let current = Some((identity, CONTEXT));
    // Two surface replies and one earlier panel message wait on a blocked renderer.
    let mut browser = StalledBrowser { pending: 3, ..StalledBrowser::default() };
    let mut queue = HostMessages::new();
    queue.push(message(identity, ping(identity)?, CONTEXT, &lease)?)?;
    let configuration = configure(identity, PanelStyle::Comfy)?;
    queue.push(message(identity, configuration.clone(), CONTEXT, &lease)?)?;
    for _ in 0..4 {
        let admitted = browser.admits_panel_delivery();
        assert!(!queue.progress(current, admitted, |value| browser.call(&value))?);
    }
    assert!(browser.submitted.is_empty());
    assert!(lease.claim(1).is_err());

    browser.pending -= 1;
    let admitted = browser.admits_panel_delivery();
    assert!(queue.progress(current, admitted, |value| browser.call(&value))?);
    let admitted = browser.admits_panel_delivery();
    assert!(!queue.progress(current, admitted, |value| browser.call(&value))?);
    browser.pending -= 1;
    let admitted = browser.admits_panel_delivery();
    assert!(queue.progress(current, admitted, |value| browser.call(&value))?);

    assert!(queue.is_empty());
    assert_eq!(
        browser.submitted,
        [
            (CONTEXT, expected(&ping(identity)?, CONTEXT)?),
            (CONTEXT, expected(&configuration, CONTEXT)?)
        ]
    );
    assert!(lease.claim(2 * PanelCommand::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn newer_message_supersedes_an_unsent_one_of_its_kind_and_releases_its_claim()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(3 * PanelCommand::STORAGE_BYTES, Box::new(Release));
    let mut queue = HostMessages::new();
    queue.push(message(identity, configure(identity, PanelStyle::Comfy)?, CONTEXT, &lease)?)?;
    queue.push(message(identity, ping(identity)?, CONTEXT, &lease)?)?;
    let latest = configure(identity, PanelStyle::Fluent)?;
    queue.push(message(identity, latest.clone(), CONTEXT, &lease)?)?;
    assert!(lease.claim(PanelCommand::STORAGE_BYTES).is_ok());

    let mut sent = Vec::new();
    for _ in 0..2 {
        assert!(queue.progress(Some((identity, CONTEXT)), true, |value| {
            sent.push(value.parameters().to_owned());
            Ok(())
        })?);
    }
    assert_eq!(sent, [expected(&latest, CONTEXT)?, expected(&ping(identity)?, CONTEXT)?]);
    Ok(())
}

#[test]
fn retained_message_for_a_superseded_document_is_retired_not_submitted()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(2 * PanelCommand::STORAGE_BYTES, Box::new(Release));
    let mut queue = HostMessages::new();
    queue.push(message(identity, ping(identity)?, CONTEXT, &lease)?)?;
    queue.push(message(identity, configure(identity, PanelStyle::Comfy)?, CONTEXT, &lease)?)?;
    let mut sent = 0;
    assert!(queue.progress(Some((identity, CONTEXT + 1)), true, |_| {
        sent += 1;
        Ok(())
    })?);
    assert_eq!(sent, 0);
    assert!(queue.is_empty());
    assert!(lease.claim(2 * PanelCommand::STORAGE_BYTES).is_ok());

    let lease = MemoryLease::new(PanelCommand::STORAGE_BYTES, Box::new(Release));
    queue.push(message(identity, ping(identity)?, CONTEXT, &lease)?)?;
    assert!(queue.progress(None, false, |_| {
        sent += 1;
        Ok(())
    })?);
    assert_eq!(sent, 0);
    assert!(queue.is_empty());
    assert!(lease.claim(PanelCommand::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn sustained_heartbeats_during_a_stalled_renderer_never_exhaust_command_slots()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(4 * PanelCommand::STORAGE_BYTES, Box::new(Release));
    let current = Some((identity, CONTEXT));
    let mut browser = StalledBrowser { pending: 2, ..StalledBrowser::default() };
    let mut queue = HostMessages::new();
    for beat in 0..64 {
        let value =
            if beat % 8 == 0 { configure(identity, PanelStyle::Comfy)? } else { ping(identity)? };
        queue.push(message(identity, value, CONTEXT, &lease)?)?;
        let admitted = browser.admits_panel_delivery();
        queue.progress(current, admitted, |value| browser.call(&value))?;
        assert!(browser.pending <= 3);
    }
    assert_eq!(browser.submitted.len(), 1);
    browser.pending = 0;
    while !queue.is_empty() {
        let admitted = browser.admits_panel_delivery();
        assert!(queue.progress(current, admitted, |value| browser.call(&value))?);
    }
    assert_eq!(browser.submitted.len(), 3);
    assert!(lease.claim(4 * PanelCommand::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn close_releases_every_retained_message_without_submission() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(2 * PanelCommand::STORAGE_BYTES, Box::new(Release));
    let mut queue = HostMessages::new();
    queue.push(message(identity, ping(identity)?, CONTEXT, &lease)?)?;
    queue.push(message(identity, configure(identity, PanelStyle::Comfy)?, CONTEXT, &lease)?)?;
    assert!(lease.claim(1).is_err());
    queue.clear();
    assert!(queue.is_empty());
    assert!(!queue.progress(Some((identity, CONTEXT)), true, |_| {
        Err(DesktopError::message("test", "cleared message was submitted"))
    })?);
    assert!(lease.claim(2 * PanelCommand::STORAGE_BYTES).is_ok());
    Ok(())
}
