// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Implement ordered startup effects and permanently closed terminal admission.

use super::{RejectedStartupEvent, SessionFailure, StartupPhase, StartupRejection};
use crate::readiness::{ReadinessDecision, ReadinessGate, ReceiptRejection};
use sugarsubstitute_contracts::{
    configuration::{ConnectionConfiguration, LoopbackOrigin, StartupConfiguration},
    execution::Generation,
    startup::{
        ConnectionCleanup, LaunchId, ReadinessOutcome, SplashIntent, StartupEffect, StartupEvent,
        StartupObservation,
    },
};

/// Own one attempt's ordering, validated configuration and frontend admission.
///
/// Methods perform no I/O, waiting, spawning or native operations. The composing
/// application delivers actual acknowledgements and executes each returned effect
/// once. `Running` requires acknowledged native reveal, not compositor visibility. Cancellation
/// requests cannot prove blocking/native work stopped. A successful retirement
/// preserves readiness; connection cleanup is reserved for failure/cancellation.
pub struct StartupSession {
    configuration: StartupConfiguration,
    readiness: ReadinessGate,
    phase: StartupPhase,
    origin: Option<LoopbackOrigin>,
    admission_closed: bool,
    failure: Option<SessionFailure>,
}

impl StartupSession {
    /// Consume explicit validated intent and positive application-assigned identities.
    /// No connection work is requested until `SplashPresented` is acknowledged.
    pub fn new(
        launch: LaunchId,
        navigation: Generation,
        configuration: StartupConfiguration,
    ) -> Self {
        Self {
            configuration,
            readiness: ReadinessGate::new(launch, navigation),
            phase: StartupPhase::SplashFirstFrame,
            origin: None,
            admission_closed: false,
            failure: None,
        }
    }

    /// Borrow the authoritative configuration; its lexical paths grant no trust.
    pub const fn configuration(&self) -> &StartupConfiguration {
        &self.configuration
    }
    /// Return the application-owned identity of the current attempt.
    pub const fn launch(&self) -> LaunchId {
        self.readiness.launch()
    }
    /// Return the only navigation eligible for frontend completion.
    pub const fn navigation(&self) -> Generation {
        self.readiness.navigation()
    }
    /// Return acknowledged startup progress, never a percentage derived from elapsed time.
    pub const fn phase(&self) -> StartupPhase {
        self.phase
    }
    /// Borrow the first failure, retained through cancellation and settlement until retry.
    pub const fn failure(&self) -> Option<&SessionFailure> {
        self.failure.as_ref()
    }
    /// Return the admitted endpoint for diagnostics; cancellation makes it non-publishable.
    pub const fn connection_origin(&self) -> Option<LoopbackOrigin> {
        self.origin
    }

    /// Admit one authoritative observation and emit at most one requested effect.
    ///
    /// # Errors
    /// Rejects foreign identities, stale/duplicate receipts, wrong endpoints and
    /// acknowledgements outside their requested phase without mutation. The error
    /// returns ownership of the event, preserving rejected native failure causes.
    /// Frontend receipt inspection clones at most its validated four-KiB payload;
    /// original adapter failure causes are transferred without cloning.
    pub fn observe(
        &mut self,
        event: StartupEvent,
    ) -> Result<Option<StartupEffect>, RejectedStartupEvent> {
        if let Err(reason) = self.validate(&event) {
            return Err(RejectedStartupEvent { reason, event });
        }
        let effect = match event.into_observation() {
            StartupObservation::SplashPresented => {
                self.phase = StartupPhase::PreparingConnection;
                Some(StartupEffect::PrepareConnection { launch: self.launch() })
            }
            StartupObservation::ConnectionReady(origin) => {
                self.origin = Some(origin);
                self.phase = StartupPhase::ServerReady;
                Some(StartupEffect::LoadFrontend {
                    launch: self.launch(),
                    navigation: self.navigation(),
                    origin,
                })
            }
            StartupObservation::FrontendNavigationStarted(_) => {
                self.phase = StartupPhase::WebviewLoading;
                None
            }
            StartupObservation::SplashRetired => {
                self.phase = StartupPhase::RevealingMain;
                Some(StartupEffect::RevealMain { launch: self.launch() })
            }
            StartupObservation::MainRevealed => {
                self.phase = StartupPhase::Running;
                None
            }
            StartupObservation::ConnectionSettled => {
                self.phase = StartupPhase::Stopped;
                None
            }
            StartupObservation::Failed(failure) => {
                self.failure = Some(SessionFailure::Adapter(failure));
                Some(self.fail())
            }
            StartupObservation::FrontendReceipt(receipt) => {
                match self.readiness.accept(receipt.clone()) {
                    ReadinessDecision::Accepted(ReadinessOutcome::Ready { .. }) => {
                        self.phase = StartupPhase::FrontendReady;
                        Some(StartupEffect::RetireSplash { launch: self.launch() })
                    }
                    ReadinessDecision::Accepted(ReadinessOutcome::Failed { failure }) => {
                        self.failure = Some(SessionFailure::Frontend(failure));
                        Some(self.fail())
                    }
                    ReadinessDecision::Rejected(reason) => {
                        let reason = match reason {
                            ReceiptRejection::ForeignLaunch => StartupRejection::ForeignLaunch,
                            ReceiptRejection::WrongNavigation => StartupRejection::WrongNavigation,
                            ReceiptRejection::AlreadyCompleted => {
                                StartupRejection::AlreadyCompleted
                            }
                            ReceiptRejection::Cancelled => StartupRejection::AdmissionClosed,
                        };
                        return Err(RejectedStartupEvent {
                            reason,
                            event: StartupEvent::new(
                                self.launch(),
                                StartupObservation::FrontendReceipt(receipt),
                            ),
                        });
                    }
                }
            }
        };
        Ok(effect)
    }

    /// Apply a launch-tagged user intent without confusing retirement with cancellation.
    ///
    /// # Errors
    /// Rejects foreign surfaces and cancellation after successful startup. Repeated
    /// cancellation or cancellation of a failed/stopped attempt emits no new cleanup.
    pub fn intent(
        &mut self,
        launch: LaunchId,
        intent: SplashIntent,
    ) -> Result<Option<StartupEffect>, StartupRejection> {
        if launch != self.launch() {
            return Err(StartupRejection::ForeignLaunch);
        }
        match intent {
            SplashIntent::Cancel => match self.phase {
                StartupPhase::Running => Err(StartupRejection::InvalidOrder),
                StartupPhase::Failed | StartupPhase::Cancelling | StartupPhase::Stopped => Ok(None),
                _ => {
                    self.close_admission();
                    self.phase = StartupPhase::Cancelling;
                    Ok(Some(self.cleanup()))
                }
            },
        }
    }

    /// Replace a settled attempt with fresh validated intent and increasing launch identity.
    ///
    /// # Errors
    /// Requires observed cleanup settlement and permanently closed old admission.
    /// Rejects reused/backwards identities without mutation. Navigation may restart
    /// at any positive generation because the launch identity changes. Preparation
    /// still waits for this new attempt's successful splash submission.
    pub fn retry(
        &mut self,
        launch: LaunchId,
        navigation: Generation,
        configuration: StartupConfiguration,
    ) -> Result<(), StartupRejection> {
        if self.phase != StartupPhase::Stopped || !self.admission_closed {
            return Err(StartupRejection::SettlementRequired);
        }
        if launch.get() <= self.launch().get() {
            return Err(StartupRejection::LaunchIdentityNotIncreasing);
        }
        *self = Self::new(launch, navigation, configuration);
        Ok(())
    }

    /// Validate all externally supplied ordering and identities before mutation.
    fn validate(&self, event: &StartupEvent) -> Result<(), StartupRejection> {
        if event.launch() != self.launch() {
            return Err(StartupRejection::ForeignLaunch);
        }
        let expected = match event.observation() {
            StartupObservation::SplashPresented => StartupPhase::SplashFirstFrame,
            StartupObservation::ConnectionReady(origin) => {
                let matches = match self.configuration.connection() {
                    ConnectionConfiguration::Launch(launch) => origin.port() == launch.port(),
                    ConnectionConfiguration::Attach(configured) => origin == configured,
                };
                if !matches {
                    return Err(StartupRejection::WrongOrigin);
                }
                StartupPhase::PreparingConnection
            }
            StartupObservation::FrontendNavigationStarted(navigation) => {
                if *navigation != self.navigation() {
                    return Err(StartupRejection::WrongNavigation);
                }
                StartupPhase::ServerReady
            }
            StartupObservation::FrontendReceipt(receipt) => {
                if receipt.launch() != self.launch() {
                    return Err(StartupRejection::ForeignLaunch);
                }
                if receipt.navigation() != self.navigation() {
                    return Err(StartupRejection::WrongNavigation);
                }
                if self.admission_closed {
                    return Err(StartupRejection::AdmissionClosed);
                }
                if matches!(
                    self.phase,
                    StartupPhase::FrontendReady
                        | StartupPhase::RevealingMain
                        | StartupPhase::Running
                ) {
                    return Err(StartupRejection::AlreadyCompleted);
                }
                StartupPhase::WebviewLoading
            }
            StartupObservation::SplashRetired => StartupPhase::FrontendReady,
            StartupObservation::MainRevealed => StartupPhase::RevealingMain,
            StartupObservation::ConnectionSettled => {
                return if self.admission_closed
                    && matches!(self.phase, StartupPhase::Failed | StartupPhase::Cancelling)
                {
                    Ok(())
                } else {
                    Err(StartupRejection::InvalidOrder)
                };
            }
            StartupObservation::Failed(_) => {
                return if !self.admission_closed && self.phase != StartupPhase::Running {
                    Ok(())
                } else {
                    Err(StartupRejection::InvalidOrder)
                };
            }
        };
        if self.phase == expected { Ok(()) } else { Err(StartupRejection::InvalidOrder) }
    }

    /// Fail permanently and request cleanup without asserting work stopped.
    fn fail(&mut self) -> StartupEffect {
        self.close_admission();
        self.phase = StartupPhase::Failed;
        self.cleanup()
    }

    /// Invalidate frontend admission before cleanup becomes observable to consumers.
    fn close_admission(&mut self) {
        self.readiness.cancel();
        self.admission_closed = true;
    }

    /// Derive cleanup policy from the original authoritative connection mode.
    fn cleanup(&self) -> StartupEffect {
        let mode = match self.configuration.connection() {
            ConnectionConfiguration::Launch(_) => ConnectionCleanup::CancelManagedPreparation,
            ConnectionConfiguration::Attach(_) => ConnectionCleanup::DisconnectAttachment,
        };
        StartupEffect::CleanupConnection { launch: self.launch(), mode }
    }
}
