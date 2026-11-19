// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Record what this host answers the SugarCubes conformance kit, from its production owners.
//!
//! The kit (the pack's `integrations/shared/conformance/HostConformance.ts`) plays the page.
//! For a host serving the panel, canvas, memory, updates and release notes protocols it sends the
//! fixed sequence in [`page_script`]. Each message goes to the owner that answers it in production:
//! the panel owner (description, configure and heartbeat), the desktop child's surface answer
//! before the native surface can answer, the memory governor, the updates link and the release
//! notes link. What they answer to each page
//! message must equal the recorded answers in `conformance_answers.json`.
//!
//! `crates/sugarsubstitute-desktop/tests/host_conformance.mjs` runs the kit under Node over the
//! real page-channel codec, answering from that file. It proves the kit sends exactly these page
//! messages and passes with these answers, and records the kit's transcript as this host's
//! SugarCubes profile, which SugarCubes replays in CI.

use super::*;
use crate::startup::{
    memory::MemoryGovernor,
    release_notes_fetch::NoFetches,
    release_notes_page::ReleaseNotesPage,
    session_notice_page::SessionNoticePage,
    updates_page::{PageRequest, UpdatesPage},
};
use serde_json::{Value, json};
use sugarsubstitute_application::updates_panel::{IdleUpdateStatus, IdleWorkflowActivity};
use sugarsubstitute_contracts::{
    canvas_surface::CanvasSurfaceMessage,
    execution::{Generation, MemoryRelease},
    host_release_notes::ReleaseNotesClientMessage,
    host_session_notice::SessionNoticeClientMessage,
    host_updates::UpdatesClientMessage,
    memory::MemoryMessage,
    startup::LaunchId,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, PanelIngressReceipt, SurfaceReply};
use sugarsubstitute_execution::memory_budget::{
    MemoryPreference, OsPressure, SystemMemory, SystemMemorySample,
};

/// What this host answered to each page message when its profile was recorded.
const RECORDED: &str = include_str!("conformance_answers.json");
/// The application version the recording was made with. The host states its own version in its
/// updates and release notes answers, and every release changes that version; the recording
/// and the SugarCubes profile made from it keep this one.
const RECORDED_VERSION: &str = "0.1.0";
const GIB: u64 = 1024 * 1024 * 1024;

/// State the recorded version wherever the host states this build's as its current version.
fn with_recorded_version(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (name, field) in fields {
                if name == "currentVersion" && field.as_str() == Some(env!("CARGO_PKG_VERSION")) {
                    *field = Value::String(RECORDED_VERSION.into());
                } else {
                    with_recorded_version(field);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(with_recorded_version),
        _ => {}
    }
}

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// A machine with 16 GiB installed, half of it free and no platform pressure.
struct Machine;
impl SystemMemory for Machine {
    fn sample(&self, _: &[u32]) -> std::io::Result<SystemMemorySample> {
        Ok(SystemMemorySample {
            total_bytes: 16 * GIB,
            available_bytes: 8 * GIB,
            commit_available_bytes: Some(16 * GIB),
            os_pressure: OsPressure::None,
            footprint_bytes: GIB,
        })
    }
}

/// The page messages the kit sends, in order, with the envelope it builds for each protocol.
fn page_script(session: &str) -> Vec<Value> {
    let envelope = |protocol: &str, version: u64| json!({ "protocol": protocol, "version": version, "session": session });
    let with = |mut value: Value, fields: Value| {
        if let (Some(value), Some(fields)) = (value.as_object_mut(), fields.as_object()) {
            value.extend(fields.clone());
        }
        value
    };
    vec![
        with(envelope("sugarcubes.panel", 3), json!({ "type": "ready" })),
        with(envelope("sugarcubes.panel", 3), json!({ "type": "applied" })),
        with(
            envelope("sugarcubes.canvas", 3),
            json!({ "requestId": 1, "type": "surface.capabilities.request", "schema": 3 }),
        ),
        with(
            envelope("sugarcubes.canvas", 3),
            json!({ "requestId": 2, "type": "surface.capabilities.request", "schema": 1003 }),
        ),
        with(
            envelope("sugarcubes.memory", 1),
            json!({ "type": "preference", "preference": { "mode": "share", "percent": 25 } }),
        ),
        with(
            envelope("sugarcubes.memory", 1),
            json!({ "type": "usage", "usedBytes": 0, "demandBytes": 0, "consumers": [] }),
        ),
        // Every released page speaks version 1; the host also answers version 2, as it declares.
        with(envelope("sugarcubes.updates", 1), json!({ "type": "watch" })),
        with(
            envelope("sugarcubes.updates", 1),
            json!({ "type": "check", "request": "conformance-check-v1" }),
        ),
        with(envelope("sugarcubes.updates", 2), json!({ "type": "watch" })),
        with(
            envelope("sugarcubes.updates", 2),
            json!({ "type": "check", "request": "conformance-check-v2" }),
        ),
        with(envelope("sugarcubes.release-notes", 1), json!({ "type": "watch" })),
        with(envelope("sugarcubes.session-notice", 1), json!({ "type": "watch" })),
        with(envelope("sugarcubes.session-notice", 2), json!({ "type": "watch" })),
    ]
}

/// The production owners that answer the kit for one composition-hosted main navigation.
struct Host {
    identity: MainIdentity,
    lease: MemoryLease,
    panel: PanelOwner,
    deliveries: Deliveries,
    memory: MemoryGovernor,
    updates: UpdatesPage,
    release_notes: ReleaseNotesPage,
    session_notice: SessionNoticePage,
    now: Moment,
}

impl Host {
    fn new() -> Result<Self> {
        let identity = MainIdentity {
            run: super::super::test_support::run()?,
            launch: LaunchId::new(1)?,
            navigation: Generation::new(1)?,
        };
        let lease = MemoryLease::new(16 * 1024 * 1024, Box::new(Release));
        let accent = Some(AccentColor::new(0, 120, 212));
        let panel = PanelOwner::new(
            identity,
            lease.clone(),
            Moment::from_nanos(0),
            accent,
            MainHosting::Composition,
        )?
        .ok_or("Panel owner was not admitted")?;
        let mut deliveries = Deliveries::new();
        deliveries.bind(identity, &lease)?;
        // The recorded host's previous close kept everything.
        let mut session_notice = SessionNoticePage::new();
        session_notice.settle(None)?;
        Ok(Self {
            identity,
            lease,
            panel,
            deliveries,
            memory: MemoryGovernor::new(Box::new(Machine), MemoryPreference::default()),
            updates: UpdatesPage::new()?,
            release_notes: ReleaseNotesPage::new()?,
            session_notice,
            now: Moment::from_nanos(1),
        })
    }

    /// Answer one page message the way the production routing hands it to its owner.
    fn answer(&mut self, message: &Value) -> Result<Vec<Value>> {
        let bytes = serde_json::to_vec(message)?;
        let claim = || self.lease.claim(PanelIngressReceipt::STORAGE_BYTES);
        let mut answers = Vec::new();
        match message["protocol"].as_str() {
            Some("sugarcubes.panel") => {
                // An applied presentation is answered at once by a connectivity ping.
                self.panel.receive(PanelMessage::decode(&bytes)?, self.now)?;
                self.drain_panel(&mut answers)?;
            }
            Some("sugarcubes.canvas") => {
                let message = CanvasSurfaceMessage::decode(&bytes)?;
                let lease = self.lease.clone();
                self.deliveries.surface(
                    self.identity,
                    message,
                    &lease,
                    lease.claim(SurfaceReply::STORAGE_BYTES)?,
                    MainHosting::Composition,
                )?;
                while let Some(command) = self.deliveries.take_command(self.identity) {
                    if let DesktopStartupCommand::SendSurface(reply) = command {
                        answers.push(serde_json::from_slice(reply.payload())?);
                    }
                }
            }
            Some("sugarcubes.memory") => {
                let message = MemoryMessage::decode(&bytes)?;
                self.memory.observe_page(self.identity, message, self.lease.clone(), claim()?)?;
                // Sample exactly when due, so no real time decides the answer.
                let due = self.memory.next_deadline();
                self.memory.poll(due, self.now, 0, false)?;
                while let Some(command) = self.memory.take_command() {
                    if let DesktopStartupCommand::SendMemory(allowance) = command {
                        answers.push(serde_json::from_slice(allowance.payload())?);
                    }
                }
            }
            Some("sugarcubes.updates") => {
                let (message, protocol) = UpdatesClientMessage::decode(&bytes)?;
                let request = PageRequest {
                    identity: self.identity,
                    message,
                    protocol,
                    lease: self.lease.clone(),
                    claim: claim()?,
                };
                self.updates.observe(request, &mut IdleUpdateStatus, &IdleWorkflowActivity)?;
                while let Some(command) = self.updates.take_command() {
                    if let DesktopStartupCommand::SendUpdates(update) = command {
                        answers.push(serde_json::from_slice(update.payload())?);
                    }
                }
            }
            Some("sugarcubes.release-notes") => {
                let message = ReleaseNotesClientMessage::decode(&bytes)?;
                self.release_notes.observe(
                    self.identity,
                    message,
                    self.lease.clone(),
                    claim()?,
                    &mut NoFetches,
                )?;
                while let Some(command) = self.release_notes.take_command() {
                    if let DesktopStartupCommand::SendReleaseNotes(notes) = command {
                        answers.push(serde_json::from_slice(notes.payload())?);
                    }
                }
            }
            Some("sugarcubes.session-notice") => {
                let message = SessionNoticeClientMessage::decode(&bytes)?;
                self.session_notice.observe(
                    self.identity,
                    message,
                    self.lease.clone(),
                    claim()?,
                )?;
                while let Some(command) = self.session_notice.take_command() {
                    if let DesktopStartupCommand::SendSessionNotice(notice) = command {
                        answers.push(serde_json::from_slice(notice.payload())?);
                    }
                }
            }
            _ => return Err("The kit sent a protocol this host does not serve".into()),
        }
        Ok(answers)
    }

    /// Encode the panel owner's pending command, its host description first.
    fn drain_panel(&mut self, answers: &mut Vec<Value>) -> Result<()> {
        while let Some(command) = self.panel.take_command() {
            if let Some(description) = command.description() {
                let session = PanelSession::for_main(self.identity)?;
                answers.push(serde_json::from_slice(&description.encode(&session)?)?);
            }
            answers.push(serde_json::from_slice(&command.message().encode()?)?);
        }
        Ok(())
    }
}

#[test]
fn the_host_answers_the_conformance_kit_as_recorded() -> Result<()> {
    let mut host = Host::new()?;
    let session = PanelSession::for_main(host.identity)?;
    let mut exchanges = Vec::new();
    for page in page_script(session.as_str()) {
        let host_answers = host.answer(&page)?;
        exchanges.push(json!({ "page": page, "host": host_answers }));
    }
    let mut answered = json!({ "session": session.as_str(), "exchanges": exchanges });
    with_recorded_version(&mut answered);
    if serde_json::from_str::<Value>(RECORDED).ok().as_ref() != Some(&answered) {
        return Err(format!(
            "The host answers the conformance kit differently from its recording. Save this as \
             crates/sugarsubstitute/src/production/child/panel/conformance_answers.json, then \
             record the SugarCubes profile with host_conformance.mjs:\n{}",
            serde_json::to_string_pretty(&answered)?
        )
        .into());
    }
    Ok(())
}

/// Every SugarCubes release up to 0.15.3 predates the host protocols. Its page never says it is
/// ready, never asks about the surface and never watches memory, updates, release notes or
/// session notices.
/// Replayed against this host, that page is sent nothing, since it listens for none of it,
/// whatever the host's own state does meanwhile. Its panel session ends as failed at the
/// startup limit instead of waiting for a page that will never answer.
#[test]
fn a_page_older_than_the_host_protocols_is_sent_nothing_and_its_panel_ends_at_the_limit()
-> Result<()> {
    fn silent(host: &mut Host) -> Result<bool> {
        let mut panel = Vec::new();
        host.drain_panel(&mut panel)?;
        Ok(panel.is_empty()
            && host.deliveries.take_command(host.identity).is_none()
            && host.memory.take_command().is_none()
            && host.updates.take_command().is_none()
            && host.release_notes.take_command().is_none()
            && host.session_notice.take_command().is_none())
    }
    let mut host = Host::new()?;
    assert!(silent(&mut host)?, "the host spoke before the page did");
    // The host's update status is read again and its memory sampled while the page stays silent.
    host.updates.refresh(&IdleUpdateStatus, &IdleWorkflowActivity)?;
    let due = host.memory.next_deadline();
    host.memory.poll(due, host.now, 0, false)?;
    assert!(silent(&mut host)?, "a page that watches nothing was sent a state");

    let second = 1_000_000_000;
    host.panel.tick(Moment::from_nanos(30 * second - 1))?;
    assert_eq!(host.panel.policy.state(), PanelState::Loading);
    host.panel.tick(Moment::from_nanos(30 * second))?;
    assert_eq!(host.panel.policy.state(), PanelState::Failed, "the handshake has a limit");
    assert!(silent(&mut host)?, "a failed handshake sends the page nothing");
    host.panel.tick(Moment::from_nanos(3_600 * second))?;
    assert!(silent(&mut host)?, "nothing is sent to a failed session later either");
    Ok(())
}
