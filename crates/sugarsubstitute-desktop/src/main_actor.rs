// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive one admitted UI-affine browser lifetime through explicit native stages.

mod admission;
mod browser_memory;
mod close_request;
#[cfg(feature = "native-composition")]
mod composition;
mod construction;
mod evidence;
mod focus;
mod frame;
#[cfg(feature = "native-composition")]
mod hosting;
mod panel;
mod placement;
mod probe_diagnostics;
mod recovery;
#[cfg(any(feature = "k1-native-qualification", test))]
mod render_preview;
mod retirement;
mod session;
mod timeout;
mod window_lifecycle;
use crate::{
    DesktopError, DesktopStartupObservation, MainWindowRequest, NativeMaterial,
    main_events::{Command, Events},
    main_readiness::MainReadiness,
    native_action::NativeAction,
    windows::Presenter,
    windows_controls::Controls,
};
use admission::Admission;
use std::rc::Rc;
use tao::{
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    window::Window,
};
use wry::{WebView, WebViewConstruction, WebViewRetirement};

#[derive(Clone, Copy, Eq, PartialEq)]
enum ConstructionStage {
    Window,
    Presenter,
    Controls,
    /// Fix composition or windowed hosting before the browser exists.
    #[cfg(feature = "native-composition")]
    Hosting,
    Begin,
    Native,
    /// Bind the composition-hosted browser before its controller is observed.
    #[cfg(feature = "native-composition")]
    Bind,
    /// Retire an unbound composition browser before rebuilding it windowed.
    #[cfg(feature = "native-composition")]
    Rebuild,
    PausedEnvironment,
    Browser,
    Live,
}

/// Every partial native output remains with this owner across errors and cancellation.
/// The outer apartment and event loop outlive actual retirement of this complete actor.
pub(crate) struct MainActor {
    request: Option<MainWindowRequest>,
    admission: Admission,
    recovery: crate::main_backend::Recovery,
    stage: ConstructionStage,
    window: Option<Rc<Window>>,
    presenter: Option<Presenter>,
    controls: Option<Controls>,
    constructor: Option<WebViewConstruction>,
    partial: Option<WebViewRetirement>,
    view: Option<WebView>,
    /// Focus reporting and Tab wrapping registered on a windowed browser's controller.
    windowed_focus: Option<focus::Focus>,
    #[cfg(feature = "native-composition")]
    composition: Option<composition::Composition>,
    #[cfg(feature = "native-composition")]
    composition_registered: bool,
    #[cfg(feature = "native-composition")]
    hosting: hosting::HostingSelection,
    #[cfg(feature = "native-composition")]
    composition_source: composition::CompositionSource,
    /// The authorized environment retained across a windowed rebuild.
    #[cfg(feature = "native-composition")]
    reused_environment:
        Option<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment>,
    panel_bridge: Option<panel::bridge::Bridge>,
    session: Option<session::Channel>,
    canvas_replies: panel::Replies,
    panel_messages: panel::HostMessages,
    panel_transport: panel::admission::Admission,
    /// Oversized panel messages and their single-use tokens, shared with the bulk channel.
    bulk: Rc<crate::main_bulk::Transfers>,
    panel_navigation_started: bool,
    receivers: Option<crate::windows_main_receivers::Receivers>,
    events: Option<Rc<Events>>,
    readiness: Option<MainReadiness>,
    revealed: bool,
    reload: crate::main_reload::ReloadAdmission,
    retired: bool,
    navigations: u64,
    material: Option<NativeMaterial>,
    native_storage: Option<sugarsubstitute_contracts::execution::MemoryClaim>,
    placement: crate::main_placement::Observations,
    failure_observation: &'static str,
    probe_diagnostics: probe_diagnostics::ProbeDiagnostics,
    #[cfg(feature = "k1-native-qualification")]
    preview: Option<render_preview::Capture>,
    #[cfg(feature = "k1-native-qualification")]
    preview_requested: bool,
}

impl MainActor {
    /// Allocate only bounded authored state, before any window or COM effect.
    pub(crate) fn new(
        request: MainWindowRequest,
        proxy: EventLoopProxy<NativeAction>,
    ) -> Result<Self, DesktopError> {
        let admission = Admission::new(request.identity());
        let recovery = crate::main_backend::Recovery::new(request.identity());
        recovery::check_storage()?;
        let native_storage = request
            .storage_lease()
            .claim(recovery::NATIVE_BYTES)
            .map_err(|cause| DesktopError::new("claim_main_native_metadata", cause))?;
        let events = Events::new(request.storage_lease(), move || {
            if let Err(cause) = proxy.send_event(NativeAction::Main) {
                tracing::error!(operation="wake_main_actor", cause=%cause,
                    "Native main actor wake failed");
            }
        })?;
        let readiness = MainReadiness::new(
            request.identity(),
            request.origin(),
            request.versions(),
            request.storage_lease(),
        )?;
        let panel_transport = panel::admission::Admission::new(request.storage_lease())?;
        let bulk_partition = request
            .storage_lease()
            .claim(crate::main_bulk::SESSION_BYTES)
            .map_err(|cause| DesktopError::new("claim_bulk_transfers", cause))?
            .into_lease();
        // The channel lives on the exact frontend root this lifetime navigates to.
        let endpoint = crate::main_bulk::Endpoint::for_root(&crate::main_navigation::root_url(
            request.origin(),
        ))?;
        let bulk = Rc::new(crate::main_bulk::Transfers::new(
            endpoint,
            bulk_partition,
            crate::windows_main_receivers::random_token,
        )?);
        Ok(Self {
            request: Some(request),
            admission,
            recovery,
            stage: ConstructionStage::Window,
            window: None,
            presenter: None,
            controls: None,
            constructor: None,
            partial: None,
            view: None,
            windowed_focus: None,
            #[cfg(feature = "native-composition")]
            composition: None,
            #[cfg(feature = "native-composition")]
            composition_registered: false,
            #[cfg(feature = "native-composition")]
            hosting: hosting::HostingSelection::new(),
            #[cfg(feature = "native-composition")]
            composition_source: composition::CompositionSource::System,
            #[cfg(feature = "native-composition")]
            reused_environment: None,
            panel_bridge: None,
            session: None,
            canvas_replies: panel::Replies::new(),
            panel_messages: panel::HostMessages::new(),
            panel_transport,
            bulk,
            panel_navigation_started: false,
            receivers: None,
            events: Some(events),
            readiness: Some(readiness),
            revealed: false,
            reload: crate::main_reload::ReloadAdmission::default(),
            retired: false,
            navigations: 0,
            material: None,
            native_storage: Some(native_storage),
            placement: crate::main_placement::Observations::default(),
            failure_observation: "None",
            probe_diagnostics: probe_diagnostics::ProbeDiagnostics::default(),
            #[cfg(feature = "k1-native-qualification")]
            preview: None,
            #[cfg(feature = "k1-native-qualification")]
            preview_requested: false,
        })
    }

    pub(crate) fn identity(&self) -> crate::MainIdentity {
        self.admission.identity()
    }
    pub(crate) fn retired(&self) -> bool {
        self.retired
    }
    /// Return only the closed kind of the actual failed native observation.
    pub(crate) fn failure_observation(&self) -> &'static str {
        self.failure_observation
    }
    /// Borrow this exact owned native surface only while its admission remains live.
    /// The activity adapter retains its own callback/window custody before observing HWND facts.
    pub(crate) fn activity_window(&self) -> Option<&Rc<Window>> {
        if self.admission.closing() || self.retired { None } else { self.window.as_ref() }
    }
    pub(crate) fn material(&self) -> Option<NativeMaterial> {
        self.material
    }
    pub(crate) fn navigations(&self) -> u64 {
        self.navigations
    }
    /// Return the selected browser hosting; an undecided lifetime is never composition-hosted.
    pub(crate) fn hosting(&self) -> crate::MainHosting {
        #[cfg(feature = "native-composition")]
        {
            self.hosting.selected().unwrap_or(crate::MainHosting::Windowed)
        }
        #[cfg(not(feature = "native-composition"))]
        {
            crate::MainHosting::Windowed
        }
    }
    /// Native cadence advances finite construction/retirement without a separate scheduler.
    pub(crate) fn needs_progress(&self) -> bool {
        !self.retired
            && (self.admission.closing()
                || (self.stage != ConstructionStage::Live
                    && self.stage != ConstructionStage::PausedEnvironment)
                || self.events.as_ref().is_some_and(|events| events.remaining() != 0)
                || self.panel_bridge_needs_progress()
                || self.session.as_ref().is_some_and(session::Channel::needs_progress)
                || self.canvas_reply_needs_progress()
                || self.panel_message_needs_progress()
                || self.panel_transport_needs_progress()
                || self.preview_needs_progress()
                || self.placement.pending(self.revealed))
    }
    fn request(&self) -> Result<&MainWindowRequest, DesktopError> {
        self.request
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_request", "request retired"))
    }
    /// Preserve a genuine completed preview wake without polling its pending native callback.
    fn preview_needs_progress(&self) -> bool {
        #[cfg(feature = "k1-native-qualification")]
        {
            self.preview.as_ref().is_some_and(render_preview::Capture::publication_pending)
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        {
            false
        }
    }
    /// Preserve a real bridge callback wake without polling a pending native registration.
    fn panel_bridge_needs_progress(&self) -> bool {
        !self.panel_navigation_started
            && self.panel_bridge.as_ref().is_some_and(panel::bridge::Bridge::needs_progress)
    }
    fn window(&self) -> Result<&Rc<Window>, DesktopError> {
        self.window
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_window", "window not created"))
    }
    fn events(&self) -> Result<&Rc<Events>, DesktopError> {
        self.events.as_ref().ok_or_else(|| DesktopError::message("main_events", "events retired"))
    }
    fn view(&self) -> Result<&WebView, DesktopError> {
        self.view
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_view", "controller not created"))
    }
    fn readiness(&mut self) -> Result<&mut MainReadiness, DesktopError> {
        self.readiness
            .as_mut()
            .ok_or_else(|| DesktopError::message("main_readiness", "readiness retired"))
    }
    fn receivers(&self) -> Result<&crate::windows_main_receivers::Receivers, DesktopError> {
        self.receivers
            .as_ref()
            .ok_or_else(|| DesktopError::message("main_receivers", "receivers not created"))
    }

    /// Advance one native stage or one immutable observation; never pump or wait.
    pub(crate) fn progress(
        &mut self,
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: EventLoopProxy<NativeAction>,
        title: &str,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        if self.retired {
            return Ok(None);
        }
        if self.admission.closing() {
            return self.retire_step();
        }
        if let Some(cause) = self.events()?.take_failure() {
            return Err(cause);
        }
        if self.constructor.as_ref().is_some_and(WebViewConstruction::take_close_requested)
            || self.view.as_ref().is_some_and(WebView::take_close_requested)
        {
            return close_request::observe(self.revealed);
        }
        if let Some(kind) = self.events()?.process_failed()
            && let Some(reason) =
                self.reload.request(self.revealed, crate::MainReloadReason::ProcessFailed)
        {
            tracing::warn!(operation="main_process_failed", identity=?self.identity(), kind,
                "Actual WebView process failure requested checked frontend replacement");
            return Ok(Some(DesktopStartupObservation::MainReloadRequested {
                identity: self.identity(),
                reason,
            }));
        }
        if let Some(cause) = self.view.as_ref().and_then(WebView::take_owned_error) {
            return Err(DesktopError::new("main_owned_callback", cause));
        }
        if self.stage != ConstructionStage::Live {
            return self.construct_step(target, proxy, title);
        }
        if let Some(observation) = self.placement_step()? {
            return Ok(Some(observation));
        }
        if let Some(session) = &mut self.session {
            if !session.installed() && !session.install_step()? {
                return Ok(None);
            }
            if let Some(observation) = session.step()? {
                return Ok(Some(observation));
            }
        }
        if self.progress_panel_transport()? || self.progress_panel_messages()? {
            return Ok(None);
        }
        let reply_was_due = self.canvas_replies.reply_due();
        if reply_was_due && self.progress_canvas_reply()? {
            return Ok(None);
        }
        let evidence = self.evidence_step()?;
        if evidence.consumed {
            self.canvas_replies.evidence_serviced();
        }
        let observed = evidence.observation;
        if observed.is_some() || self.events()?.remaining() != 0 {
            return Ok(observed);
        }
        if !reply_was_due && !self.canvas_replies.is_empty() && self.progress_canvas_reply()? {
            return Ok(None);
        }
        if !self.panel_navigation_started
            && let Some(bridge) = &mut self.panel_bridge
            && bridge.install_step()?
        {
            self.panel_navigation_started = true;
            return self.navigate_frontend();
        }
        #[cfg(feature = "k1-native-qualification")]
        {
            Ok(self.preview_progress())
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        {
            Ok(observed)
        }
    }

    /// Reject stale authorizations before invoking either native stage.
    pub(crate) fn authorize_environment(
        &mut self,
        identity: crate::MainIdentity,
    ) -> Result<(), DesktopError> {
        self.admission.authorize_environment(identity)?;
        self.constructor
            .as_mut()
            .ok_or_else(|| DesktopError::message("authorize_environment", "constructor missing"))?
            .authorize_environment()
            .map_err(|cause| DesktopError::new("authorize_environment", cause))?;
        self.stage = ConstructionStage::Native;
        Ok(())
    }
    pub(crate) fn authorize_browser(
        &mut self,
        identity: crate::MainIdentity,
    ) -> Result<(), DesktopError> {
        self.admission.authorize_browser(identity)?;
        self.stage = ConstructionStage::Browser;
        #[cfg(feature = "k1-native-qualification")]
        tracing::info!(operation="main_browser_authorized", identity=?identity,
            "Actual Main browser stage authorized");
        #[cfg(feature = "k1-native-qualification")]
        crate::qualification_trace::progress(
            crate::qualification_trace::Progress::BrowserAuthorized,
            identity,
        );
        Ok(())
    }

    /// Request one actual current document sample, preserving any pending sample's identity.
    pub(crate) fn probe(&mut self, identity: crate::MainIdentity) -> Result<(), DesktopError> {
        self.admission.frontend(identity)?;
        if let Some(target) =
            self.readiness()?.poll_probe(std::time::Instant::now()).map_err(readiness_error)?
        {
            let parameters = evidence::probe_parameters(target, self.request()?)?;
            self.receivers()?.call(Command::Probe(target), "Runtime.evaluate", &parameters)?;
            self.probe_diagnostics.submitted(target);
        }
        Ok(())
    }

    /// Return the current incomplete sample wake for the outer native event loop.
    pub(crate) fn next_probe_deadline(&self) -> Option<std::time::Instant> {
        self.readiness
            .as_ref()
            .and_then(MainReadiness::next_probe_deadline)
            .into_iter()
            .chain(self.session.as_ref().and_then(session::Channel::deadline))
            .min()
    }
}

fn readiness_error(cause: crate::main_readiness_types::ReadinessFault) -> DesktopError {
    DesktopError::new("main_frontend_evidence", cause)
}
