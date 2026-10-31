// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual native facts into the sole application startup policy.

use std::{error::Error, fmt};
use sugarsubstitute_application::startup::{StartupPhase, StartupSession};
use sugarsubstitute_contracts::{
    lifecycle::RunKey,
    startup::{
        LaunchId, MainIdentity, SplashIntent, StartupEffect, StartupEvent, StartupObservation,
    },
};
use sugarsubstitute_desktop::DesktopStartupObservation;

/// Transfer an unchanged native fact or the single authoritative application effect.
/// One synchronous route moves the native payload into the child owner without allocating.
#[allow(
    clippy::large_enum_variant,
    reason = "Keep one native observation inline during its synchronous move into child custody; boxing adds an unadmitted allocation"
)]
pub(super) enum StartupRoute {
    Native(DesktopStartupObservation),
    Effect(StartupEffect),
    Close,
    None,
}

/// Reject another run/attempt without dropping its actual native observation.
pub(super) struct RejectedNativeObservation {
    observation: DesktopStartupObservation,
}
impl fmt::Debug for RejectedNativeObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RejectedNativeObservation")
    }
}
impl fmt::Display for RejectedNativeObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Native startup observation belongs to another run or attempt")
    }
}
impl Error for RejectedNativeObservation {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.observation {
            DesktopStartupObservation::Failed { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

/// Keep presentation ordering separate from retained child/process authority.
/// Borrowed session intent stays in its complete guarded configuration owner.
/// Bootstrap/capture admission remains child-owned; absent policy grants no UI effect.
pub(super) struct StartupPolicy {
    run: RunKey,
    launch: LaunchId,
    closing: bool,
}
impl StartupPolicy {
    /// Retain inert original role identities without constructing application policy.
    pub(super) fn new(run: RunKey, launch: LaunchId) -> Self {
        Self { run, launch, closing: false }
    }
    /// Return inert original launch identity before actual guarded policy is available.
    pub(super) const fn launch(&self) -> LaunchId {
        self.launch
    }
    /// Return the immutable run; matching data does not establish authentication.
    pub(super) const fn run(&self) -> RunKey {
        self.run
    }
    /// Return the current exact main identity used to reject stale native callbacks.
    pub(super) fn identity(&self, session: &StartupSession) -> MainIdentity {
        MainIdentity { run: self.run, launch: session.launch(), navigation: session.navigation() }
    }
    /// Close ordinary presentation admission once; running close is role retirement.
    /// Without a prepared session, the actual child retires partial startup independently.
    /// # Errors
    /// Preserve an application rejection without manufacturing cleanup settlement.
    pub(super) fn close(
        &mut self,
        session: Option<&mut StartupSession>,
    ) -> Result<Option<StartupEffect>, Box<dyn Error>> {
        if self.closing {
            return Ok(None);
        }
        self.closing = true;
        let Some(session) = session else { return Ok(None) };
        if session.phase() == StartupPhase::Running {
            return Ok(None);
        }
        Ok(session.intent(session.launch(), SplashIntent::Cancel)?)
    }
    /// Return whether a close request has permanently fenced ordinary presentation.
    pub(super) const fn closing(&self) -> bool {
        self.closing
    }
    /// Deliver a genuine child-owner event; the application retains all ordering checks.
    /// # Errors
    /// Returns the unchanged rejected event and its actual typed cause.
    pub(super) fn observe(
        &mut self,
        event: StartupEvent,
        session: &mut StartupSession,
    ) -> Result<StartupRoute, Box<dyn Error>> {
        Ok(match session.observe(event)? {
            Some(effect) => StartupRoute::Effect(effect),
            None => StartupRoute::None,
        })
    }
    /// Route full-key native observations; numeric folder/PID facts stay unprivileged.
    /// Closing consumes matching presentation facts; full native retirement
    /// still reaches the child owner that holds shutdown settlement.
    /// # Errors
    /// Rejects foreign identity or application ordering before granting any UI command.
    pub(super) fn native(
        &mut self,
        observation: DesktopStartupObservation,
        session: Option<&mut StartupSession>,
    ) -> Result<StartupRoute, Box<dyn Error>> {
        if !self.matches(session.as_deref(), &observation) {
            return Err(Box::new(RejectedNativeObservation { observation }));
        }
        let Some(session) = session else {
            return Ok(match observation {
                DesktopStartupObservation::CloseRequested => StartupRoute::Close,
                other => StartupRoute::Native(other),
            });
        };
        if self.closing {
            return Ok(match observation {
                DesktopStartupObservation::CloseRequested => StartupRoute::Close,
                DesktopStartupObservation::MainReloadRequested { .. }
                | DesktopStartupObservation::PanelMessage(_)
                | DesktopStartupObservation::SurfaceMessage(_)
                | DesktopStartupObservation::CanvasContextMessage(_)
                | DesktopStartupObservation::SplashPresented(_)
                | DesktopStartupObservation::SplashRetired(_)
                | DesktopStartupObservation::NavigationStarted(_)
                | DesktopStartupObservation::FrontendReceipt { .. }
                | DesktopStartupObservation::MainRevealed(_) => StartupRoute::None,
                #[cfg(feature = "native-composition")]
                DesktopStartupObservation::CanvasInput(_) => StartupRoute::None,
                other => StartupRoute::Native(other),
            });
        }
        let fact = match observation {
            DesktopStartupObservation::SplashPresented(launch) => {
                return self.observe(
                    StartupEvent::new(launch, StartupObservation::SplashPresented),
                    session,
                );
            }
            DesktopStartupObservation::NavigationStarted(identity) => {
                StartupObservation::FrontendNavigationStarted(identity.navigation)
            }
            DesktopStartupObservation::FrontendReceipt { receipt, .. } => {
                StartupObservation::FrontendReceipt(receipt)
            }
            DesktopStartupObservation::SplashRetired(launch) => {
                return self.observe(
                    StartupEvent::new(launch, StartupObservation::SplashRetired),
                    session,
                );
            }
            DesktopStartupObservation::MainRevealed(_) => StartupObservation::MainRevealed,
            DesktopStartupObservation::CloseRequested => return Ok(StartupRoute::Close),
            DesktopStartupObservation::NativeRetired(_) => {
                return Ok(StartupRoute::Native(observation));
            }
            DesktopStartupObservation::Failed { failure, .. }
                if !self.closing && session.phase() != StartupPhase::Running =>
            {
                StartupObservation::Failed(failure)
            }
            other => return Ok(StartupRoute::Native(other)),
        };
        self.observe(StartupEvent::new(session.launch(), fact), session)
    }
    fn matches(
        &self,
        session: Option<&StartupSession>,
        observation: &DesktopStartupObservation,
    ) -> bool {
        match observation {
            DesktopStartupObservation::PanelMessage(observed) => {
                session.is_some_and(|session| observed.identity() == self.identity(session))
            }
            DesktopStartupObservation::SurfaceMessage(observed) => {
                session.is_some_and(|session| observed.identity() == self.identity(session))
            }
            DesktopStartupObservation::CanvasContextMessage(observed) => {
                session.is_some_and(|session| observed.identity() == self.identity(session))
            }
            #[cfg(feature = "native-composition")]
            DesktopStartupObservation::CanvasInput(input) => {
                session.is_some_and(|session| input.identity == self.identity(session))
            }
            DesktopStartupObservation::NativeActivity { run, .. }
            | DesktopStartupObservation::NativeRetired(run) => *run == self.run,
            DesktopStartupObservation::UiChallengeProcessed(key) => key.run == self.run,
            DesktopStartupObservation::SplashPresented(launch)
            | DesktopStartupObservation::SplashRetired(launch) => {
                session.is_some_and(|session| *launch == session.launch())
            }
            DesktopStartupObservation::EnvironmentPrepared { identity, .. }
            | DesktopStartupObservation::ControllerPrepared { identity, .. }
            | DesktopStartupObservation::NavigationStarted(identity)
            | DesktopStartupObservation::FrontendReceipt { identity, .. }
            | DesktopStartupObservation::MainRevealed(identity)
            | DesktopStartupObservation::MainRetired(identity)
            | DesktopStartupObservation::MainAbsent(identity)
            | DesktopStartupObservation::BackendRetryRequested { identity, .. }
            | DesktopStartupObservation::MainReloadRequested { identity, .. } => {
                session.is_some_and(|session| *identity == self.identity(session))
            }
            DesktopStartupObservation::Failed { identity, .. } => identity.is_none_or(|identity| {
                session.is_some_and(|session| identity == self.identity(session))
            }),
            #[cfg(all(windows, feature = "k1-native-qualification"))]
            DesktopStartupObservation::MainPreviewCaptured(_)
            | DesktopStartupObservation::MainPreviewFailed { .. } => false,
            DesktopStartupObservation::CloseRequested => true,
        }
    }
}

#[cfg(test)]
#[path = "policy/tests.rs"]
pub(super) mod tests;
