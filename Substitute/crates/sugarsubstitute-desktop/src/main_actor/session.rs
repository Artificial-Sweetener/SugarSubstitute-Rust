// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one dedicated native workspace bridge through callback/script/receiver retirement.
mod handler;
mod messages;
mod queue;
mod registration;
use crate::{
    DesktopError, DesktopStartupObservation, MainIdentity, SessionBootstrap, SessionCommand,
    main_events::Events,
    main_session_contract::{contract_error, origin_text},
};
use std::rc::Rc;
use sugarsubstitute_contracts::{
    desktop_session::SessionError,
    execution::{MemoryClaim, MemoryLease},
};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
type Media = crate::windows_main_receivers::session_media::Media;
const SCRIPT: &str = include_str!("session/bridge.js");

/// Every native interface remains on the creating UI apartment, with independent session credit.
pub(super) struct Channel {
    identity: MainIdentity,
    bootstrap: SessionBootstrap,
    core: ICoreWebView2,
    events: Rc<Events>,
    queue: Rc<queue::Queue>,
    registration: registration::Registration,
    handler: Option<i64>,
    uncertain: bool,
    closed: bool,
    ready: bool,
    latest_ticket: u64,
    lease: MemoryLease,
    memory: MemoryLease,
    media: Option<Rc<Media>>,
    _memory: MemoryClaim,
}
impl Channel {
    /// Construct bounded authored storage before any native registration or navigation.
    pub(super) fn new(
        identity: MainIdentity,
        bootstrap: SessionBootstrap,
        core: ICoreWebView2,
        events: Rc<Events>,
        lease: &MemoryLease,
    ) -> Result<Self, DesktopError> {
        let memory = lease
            .claim(crate::MainWindowRequest::SESSION_STORAGE_BYTES)
            .map_err(|cause| DesktopError::new("claim_session_partition", cause))?
            .into_lease();
        let claim = memory
            .claim(128 * 1024)
            .map_err(|cause| DesktopError::new("claim_session_native", cause))?;
        let queue = queue::Queue::new(events.clone(), &memory)?;
        let media = if bootstrap.media_enabled() {
            Some(Rc::new(Media::new(identity, bootstrap.clone(), events.clone(), &memory)?))
        } else {
            None
        };
        Ok(Self {
            identity,
            bootstrap,
            core,
            events: events.clone(),
            queue,
            registration: registration::Registration::new(events, &memory)?,
            handler: None,
            uncertain: false,
            closed: false,
            ready: false,
            latest_ticket: 0,
            lease: lease.clone(),
            memory,
            media,
            _memory: claim,
        })
    }
    /// Borrow the sole native retained-media owner for the existing resource handler.
    pub(super) fn media(&self) -> Option<Rc<Media>> {
        self.media.clone()
    }
    pub(super) const fn installed(&self) -> bool {
        self.ready
    }
    /// Install at most one native registration per normal event-loop drain.
    pub(super) fn install_step(&mut self) -> Result<bool, DesktopError> {
        if self.closed || self.uncertain {
            return Err(DesktopError::message(
                "install_session_bridge",
                "native registration closed",
            ));
        }
        if self.handler.is_none() {
            self.install_handler()?;
            return Ok(false);
        }
        if !self.registration.submitted() {
            let mut descriptor = serde_json::json!({ "scope": self.bootstrap.scope(),
                "origin": origin_text(self.bootstrap.origin()), "webview": self.bootstrap.webview() });
            if let Some(media) = &self.media {
                descriptor["mediaPath"] = media.path().into();
            }
            let source = format!("{}({descriptor});", SCRIPT.trim_end());
            self.registration.submit(&self.core, &source)?;
            return Ok(false);
        }
        self.ready = self.registration.observe()?;
        Ok(self.ready)
    }
    /// Actual callback/queued request work; pending callbacks alone never cause a busy poll.
    pub(super) fn needs_progress(&self) -> bool {
        self.queue.pending()
            || self.registration.completed()
            || (!self.closed && (!self.registration.submitted() || self.handler.is_none()))
            || self
                .media
                .as_ref()
                .is_some_and(|media| media.needs_progress(std::time::Instant::now()))
    }
    /// Deliver one native-admitted observation and acknowledge only its transport admission.
    pub(super) fn step(&mut self) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if let Some(media) = &self.media {
            let progress = media.step(std::time::Instant::now())?;
            if progress.consumed {
                return Ok(progress.observation);
            }
        }
        if !self.ready || self.closed {
            return Ok(None);
        }
        let queue = self.queue.clone();
        let Some(decoded) =
            queue.consume(|source, payload| self.decode(source, payload))?.flatten()
        else {
            return Ok(None);
        };
        let mut observation = decoded.observation;
        if decoded.ticket <= self.latest_ticket {
            observation = None;
        } else {
            self.latest_ticket = decoded.ticket;
        }
        self.post(
            &serde_json::json!({ "sessionTransport": 1, "webview": self.bootstrap.webview(),
            "ticket": decoded.ticket, "accepted": observation.is_some() }),
        )?;
        Ok(observation.map(DesktopStartupObservation::SessionMessage))
    }
    /// Post one current host operation using the independent admitted command storage.
    pub(super) fn send(&self, command: SessionCommand) -> Result<(), DesktopError> {
        if command.identity() != self.identity || self.closed || !self.ready {
            return Err(contract_error("send_session_message", SessionError::Stale));
        }
        self.bootstrap
            .validate(command.message())
            .map_err(|cause| contract_error("send_session_message", cause))?;
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Posted<'a> {
            session_transport: u8,
            webview: &'a str,
            message: &'a sugarsubstitute_contracts::desktop_session::wire::SessionMessage,
        }
        self.post(&Posted {
            session_transport: 1,
            webview: self.bootstrap.webview(),
            message: command.message(),
        })
    }
    /// Send a worker's immutable media response to its exact native deferral.
    pub(super) fn media_reply(&self, reply: crate::SessionMediaReply) -> Result<(), DesktopError> {
        if let Some(media) = &self.media {
            media.reply(reply)
        } else {
            Err(DesktopError::message("session_media_reply", "media capability is absent"))
        }
    }
    /// Earliest unchanged native request deadline, if any.
    pub(super) fn deadline(&self) -> Option<std::time::Instant> {
        self.media.as_ref().and_then(|media| media.deadline())
    }
    /// Fence new input immediately while preserving all pending callbacks/deferrals for retirement.
    pub(super) fn close(&mut self) {
        self.closed = true;
        self.queue.close();
        if let Some(media) = &self.media {
            media.close();
        }
    }
    /// Remove each actual registration only after its callback and resource obligations settle.
    pub(super) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        self.close();
        if self.media.as_ref().is_some_and(|media| !media.retired()) {
            return Ok(false);
        }
        if !self.registration.retire(&self.core)? {
            return Ok(false);
        }
        if self.uncertain {
            return Ok(false);
        }
        if let Some(token) = self.handler {
            // SAFETY: This exact native token belongs to the retained creating-thread core.
            unsafe { self.core.remove_WebMessageReceived(token) }
                .map_err(|cause| DesktopError::new("remove_session_receiver", cause))?;
            self.handler = None;
        }
        Ok(Rc::strong_count(&self.queue) == 1 && !self.events.in_callback())
    }
}

impl super::MainActor {
    pub(crate) fn send_session(&mut self, command: SessionCommand) -> Result<(), DesktopError> {
        self.admission.frontend(command.identity())?;
        self.session
            .as_ref()
            .ok_or_else(|| DesktopError::message("send_session_message", "session bridge absent"))?
            .send(command)
    }
    pub(crate) fn send_session_media(
        &mut self,
        reply: crate::SessionMediaReply,
    ) -> Result<(), DesktopError> {
        if reply.identity() != self.identity() {
            return Ok(());
        }
        self.session.as_ref().map_or(Ok(()), |session| session.media_reply(reply))
    }
}
