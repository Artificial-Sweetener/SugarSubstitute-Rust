// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain current-document health through a native same-session evaluation barrier.
//!
//! This owner consumes facts from scoped native callbacks. It does not prove those
//! callbacks' actual delivery order, frontend functionality or compositor visibility.

use crate::{
    DesktopError,
    host_contract::{MainIdentity, ServedFrontendVersions},
    main_navigation::admits_url,
    main_readiness_storage::{ContextRecord, DocumentTables, FrameRecord},
    main_readiness_types::{ConsoleArgument, ConsoleKind, DocumentId, ProbeTarget, ReadinessFault},
};
use std::time::{Duration, Instant};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::MemoryLease,
    startup::{ReadinessOutcome, ReadinessReceipt},
};

#[cfg(any(target_os = "windows", test))]
#[path = "main_readiness_snapshot.rs"]
pub(crate) mod snapshot;

/// Bound repeated incomplete DOM samples without delaying their native callbacks.
const INCOMPLETE_PROBE_INTERVAL: Duration = Duration::from_millis(100);

/// Retain one incomplete current-document sample until its native retry deadline.
#[derive(Clone, Copy)]
struct RetryProbe {
    target: ProbeTarget,
    until: Instant,
}

struct PendingProbe {
    target: ProbeTarget,
    receipt: Option<ReadinessReceipt>,
    configuring_frame: bool,
}

/// Keep bounded native observation state for exactly one admitted main navigation.
pub(crate) struct MainReadiness {
    identity: MainIdentity,
    origin: LoopbackOrigin,
    versions: [DocumentId; 2],
    tables: DocumentTables,
    root: Option<FrameRecord>,
    default_context: Option<i64>,
    started: bool,
    loaded: bool,
    fault: Option<ReadinessFault>,
    pending: Option<PendingProbe>,
    next_operation: u64,
    retry_probe: Option<RetryProbe>,
    completed: bool,
    positive: bool,
}

impl MainReadiness {
    /// Return the completed positive native evidence barrier for reveal admission.
    pub(crate) fn ready(&self) -> bool {
        self.completed && self.positive && self.fault.is_none()
    }

    /// Borrow only the current verified top-document context for public panel attribution.
    /// This correlation does not grant filesystem, network or native operation authority.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn panel_context(&self) -> Option<i64> {
        if self.ready() && self.loaded { self.default_context } else { None }
    }

    /// Observe immutable current document facts after its genuine positive completion barrier.
    #[cfg(any(feature = "k1-native-qualification", test))]
    pub(crate) fn render_document(&self) -> Option<crate::main_readiness_types::RenderDocument> {
        if !self.ready() || !self.loaded {
            return None;
        }
        let root = self.root?;
        Some(crate::main_readiness_types::RenderDocument {
            identity: self.identity,
            context: self.default_context?,
            frame: root.id,
            loader: root.loader,
        })
    }
    /// Claim exact known table capacities before allocation; opaque engine storage is separate.
    pub(crate) fn new(
        identity: MainIdentity,
        origin: LoopbackOrigin,
        versions: &ServedFrontendVersions,
        lease: &MemoryLease,
    ) -> Result<Self, DesktopError> {
        let frontend = DocumentId::new(versions.frontend())
            .map_err(|cause| DesktopError::new("validate_readiness_versions", cause))?;
        let sugarcubes = DocumentId::new(versions.sugarcubes())
            .map_err(|cause| DesktopError::new("validate_readiness_versions", cause))?;
        Ok(Self {
            identity,
            origin,
            versions: [frontend, sugarcubes],
            tables: DocumentTables::new(lease)?,
            root: None,
            default_context: None,
            started: false,
            loaded: false,
            fault: None,
            pending: None,
            next_operation: 1,
            retry_probe: None,
            completed: false,
            positive: false,
        })
    }

    /// Begin only after the native Runtime and Page enablement callbacks both succeeded.
    /// About:blank setup observations are not admitted as frontend document facts.
    pub(crate) fn navigation_started(&mut self) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if self.started {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        self.started = true;
        Ok(())
    }

    /// Observe a native root/child frame without trusting a page-supplied root claim.
    pub(crate) fn frame(
        &mut self,
        id: &str,
        loader: &str,
        parent: Option<&str>,
        url: &str,
    ) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started {
            return Ok(());
        }
        let record = FrameRecord {
            id: self.id(id)?,
            loader: self.id(loader)?,
            root: parent.is_none(),
            origin_matches: admits_url(self.origin, url),
        };
        if record.root {
            if !record.origin_matches {
                return self.reject(ReadinessFault::WrongOrigin);
            }
            if self.root.is_some_and(|old| old.id != record.id || old.loader != record.loader) {
                return self.reject(ReadinessFault::ContextLost);
            }
            self.root = Some(record);
        } else if let Some(parent) = parent {
            self.id(parent)?;
        }
        if let Err(fault) = self.tables.insert_frame(record) {
            return self.reject(fault);
        }
        self.bind_default_context()
    }

    /// Retain native execution-context attribution; subframe/isolated contexts are not top evidence.
    pub(crate) fn context(
        &mut self,
        id: i64,
        frame: &str,
        origin: &str,
        default: bool,
    ) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started {
            return Ok(());
        }
        if !(1..=i64::from(i32::MAX)).contains(&id) || origin.len() > 4096 {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        let record = ContextRecord {
            id,
            frame: self.id(frame)?,
            default,
            origin: LoopbackOrigin::parse(origin, self.origin.port().get()).ok(),
        };
        if let Err(fault) = self.tables.insert_context(record) {
            return self.reject(fault);
        }
        self.bind_default_context()
    }

    /// Invalidate current evidence on actual context loss, including while a barrier is pending.
    pub(crate) fn context_destroyed(&mut self, context: i64) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started {
            return Ok(());
        }
        if self.tables.context(context).is_none() {
            // The initial navigation also destroys about:blank contexts whose
            // creation preceded admission. They never supplied current evidence.
            if self.default_context.is_none() && self.pending.is_none() && !self.loaded {
                return Ok(());
            }
            return self.reject(ReadinessFault::ContextLost);
        }
        if self.default_context == Some(context) {
            return self.reject(ReadinessFault::ContextLost);
        }
        self.tables.contexts.retain(|record| record.id != context);
        Ok(())
    }

    /// Initial old-document clearing precedes evidence; current top-context clearing invalidates it.
    pub(crate) fn contexts_cleared(&mut self) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if self.default_context.is_some() || self.pending.is_some() || self.loaded {
            return self.reject(ReadinessFault::ContextLost);
        }
        self.tables.contexts.clear();
        Ok(())
    }

    /// Native root detachment invalidates evidence; child contexts retain attribution until destroyed.
    pub(crate) fn frame_detached(&mut self, frame: &str) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started {
            return Ok(());
        }
        let id = self.id(frame)?;
        if self.root.is_some_and(|root| root.id == id) {
            return self.reject(ReadinessFault::ContextLost);
        }
        if self.tables.frame(id).is_none() && (self.default_context.is_some() || self.loaded) {
            return self.reject(ReadinessFault::ContextLost);
        }
        self.tables.frames.retain(|record| record.id != id);
        Ok(())
    }

    /// Sticky malformed/overflow/receiver failure cannot be repaired by a later positive DOM sample.
    pub(crate) fn invalidate(&mut self, fault: ReadinessFault) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
        self.pending = None;
        self.retry_probe = None;
    }

    /// Retain the two exact primitive diagnostics emitted by the inspected frontend dispatcher.
    /// The native adapter admits the console envelope; unrelated text carries no readiness state.
    pub(crate) fn console(
        &mut self,
        context: i64,
        kind: ConsoleKind,
        argument: ConsoleArgument<'_>,
    ) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started {
            return Ok(());
        }
        let Some(record) = self.tables.context(context) else {
            return self.reject(ReadinessFault::ContextLost);
        };
        let Some(root) = self.root else {
            return self.reject(ReadinessFault::ContextLost);
        };
        if !record.default || record.frame != root.id {
            return Ok(());
        }
        if record.origin != Some(self.origin) || self.default_context != Some(context) {
            return self.reject(ReadinessFault::WrongOrigin);
        }
        let ConsoleArgument::PrimitiveString(message) = argument else {
            return Ok(());
        };
        if kind == ConsoleKind::Error && message == crate::main_console::SETUP_FAILED {
            return self.reject(ReadinessFault::SetupFailed);
        }
        if kind == ConsoleKind::Warning && message == crate::main_console::DISABLED {
            return self.reject(ReadinessFault::DisabledExtension);
        }
        Ok(())
    }

    /// Record actual successful native navigation completion before scheduling a DOM probe.
    pub(crate) fn navigation_completed(&mut self, succeeded: bool) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.started || !succeeded {
            return self.reject(ReadinessFault::NativeNavigationFailed);
        }
        #[cfg(feature = "k1-native-qualification")]
        if !self.loaded {
            tracing::info!(operation="main_navigation_completed", identity=?self.identity,
                "Actual successful native Main navigation accepted");
            crate::qualification_trace::progress(
                crate::qualification_trace::Progress::NavigationCompleted,
                self.identity,
            );
        }
        self.loaded = true;
        Ok(())
    }

    /// Poll this exact document without replacing an issued evaluation or its native barrier.
    /// `now` belongs to the native event-loop clock. An incomplete result waits for
    /// its stored retry wake; issued evaluations and barriers retain their exact custody.
    /// # Errors
    /// Rejects unhealthy or stale pending document facts before coalescing a repeated poll.
    pub(crate) fn poll_probe(
        &mut self,
        now: Instant,
    ) -> Result<Option<ProbeTarget>, ReadinessFault> {
        self.check_health()?;
        if let Some(pending) = &self.pending {
            self.check_target(pending.target)?;
            return Ok(None);
        }
        if let Some(retry) = self.retry_probe {
            if let Err(fault) = self.check_document(retry.target) {
                return self.reject(fault);
            }
            if now < retry.until {
                return Ok(None);
            }
        }
        self.issue_probe()
    }

    /// Return the native retry wake only after an incomplete current-document result.
    pub(crate) fn next_probe_deadline(&self) -> Option<Instant> {
        self.retry_probe.map(|retry| retry.until)
    }

    /// Construct one new operation only when no actual evaluation or barrier is issued.
    fn issue_probe(&mut self) -> Result<Option<ProbeTarget>, ReadinessFault> {
        self.check_health()?;
        if self.pending.is_some() {
            return Err(ReadinessFault::ProbeBusy);
        }
        if self.completed || !self.loaded {
            return Ok(None);
        }
        let (Some(root), Some(context)) = (self.root, self.default_context) else {
            return Ok(None);
        };
        let Some(next) = self.next_operation.checked_add(1) else {
            return self.reject(ReadinessFault::IdentityExhausted);
        };
        let target = ProbeTarget {
            identity: self.identity,
            operation: self.next_operation,
            context,
            frame: root.id,
            loader: root.loader,
        };
        self.next_operation = next;
        self.pending = Some(PendingProbe { target, receipt: None, configuring_frame: false });
        self.retry_probe = None;
        Ok(Some(target))
    }

    /// Retain a validated positive DOM receipt until the same-session native completion barrier.
    /// `None` releases this evaluation slot and schedules its next sample one hundred
    /// milliseconds after the injected native callback-consumption time.
    pub(crate) fn dom_result(
        &mut self,
        target: ProbeTarget,
        receipt: Option<ReadinessReceipt>,
        now: Instant,
    ) -> Result<bool, ReadinessFault> {
        self.check_target(target)?;
        if self.pending.as_ref().is_some_and(|pending| pending.configuring_frame) {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        let Some(receipt) = receipt else {
            let Some(next) = now.checked_add(INCOMPLETE_PROBE_INTERVAL) else {
                return self.reject(ReadinessFault::ObservationOverflow);
            };
            self.pending = None;
            self.retry_probe = Some(RetryProbe { target, until: next });
            return Ok(false);
        };
        if receipt.launch() != self.identity.launch
            || receipt.navigation() != self.identity.navigation
        {
            return self.reject(ReadinessFault::ForeignReceipt);
        }
        if let ReadinessOutcome::Ready { evidence } = receipt.outcome()
            && (!self.versions[0].matches(&evidence.frontend_version)
                || !self.versions[1].matches(&evidence.sugarcubes_version))
        {
            return self.reject(ReadinessFault::VersionMismatch);
        }
        let Some(pending) = &mut self.pending else {
            return Err(ReadinessFault::StaleProbe);
        };
        if pending.receipt.is_some() {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        pending.receipt = Some(receipt);
        self.retry_probe = None;
        Ok(true)
    }

    /// Transfer this exact issued sample to one separate presentation operation.
    /// A configuration request carries no frontend-ready evidence or native privilege.
    pub(crate) fn begin_frame_configuration(
        &mut self,
        target: ProbeTarget,
    ) -> Result<(), ReadinessFault> {
        self.check_target(target)?;
        let Some(pending) = self.pending.as_mut() else {
            return Err(ReadinessFault::StaleProbe);
        };
        if pending.receipt.is_some() || pending.configuring_frame {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        pending.configuring_frame = true;
        Ok(())
    }

    /// Release only the matching completed mutation, then require a fresh read-only DOM sample.
    pub(crate) fn complete_frame_configuration(
        &mut self,
        target: ProbeTarget,
        now: Instant,
    ) -> Result<(), ReadinessFault> {
        self.check_target(target)?;
        let Some(pending) = self.pending.as_mut() else {
            return Err(ReadinessFault::StaleProbe);
        };
        if !pending.configuring_frame {
            return self.reject(ReadinessFault::MalformedObservation);
        }
        pending.configuring_frame = false;
        self.dom_result(target, None, now).map(|_| ())
    }

    /// Transfer only after prior native notifications have drained and the current target is rechecked.
    /// The native adapter supplies the actual same-CDP-session evaluation completion.
    pub(crate) fn barrier_complete(
        &mut self,
        target: ProbeTarget,
    ) -> Result<ReadinessReceipt, ReadinessFault> {
        self.check_target(target)?;
        let Some(receipt) = self.pending.as_mut().and_then(|pending| pending.receipt.take()) else {
            return self.reject(ReadinessFault::MissingDomResult);
        };
        self.pending = None;
        self.completed = true;
        self.positive = matches!(receipt.outcome(), ReadinessOutcome::Ready { .. });
        #[cfg(feature = "k1-native-qualification")]
        tracing::info!(operation="main_frontend_receipt", identity=?self.identity,
            ready=self.positive, probe_operation=target.operation,
            "Actual current frontend receipt transferred after native completion barrier");
        #[cfg(feature = "k1-native-qualification")]
        crate::qualification_trace::progress(
            if self.positive {
                crate::qualification_trace::Progress::FrontendReady
            } else {
                crate::qualification_trace::Progress::FrontendFailed
            },
            self.identity,
        );
        Ok(receipt)
    }

    fn check_health(&self) -> Result<(), ReadinessFault> {
        self.fault.map_or(Ok(()), Err)
    }
    fn reject<T>(&mut self, fault: ReadinessFault) -> Result<T, ReadinessFault> {
        self.invalidate(fault);
        Err(self.fault.unwrap_or(fault))
    }
    fn id(&mut self, value: &str) -> Result<DocumentId, ReadinessFault> {
        DocumentId::new(value).or_else(|fault| self.reject(fault))
    }
    fn check_target(&mut self, target: ProbeTarget) -> Result<(), ReadinessFault> {
        self.check_health()?;
        if !self.pending.as_ref().is_some_and(|pending| pending.target == target) {
            return Err(ReadinessFault::StaleProbe);
        }
        self.check_document(target)
    }
    /// Validate the current native document independently of callback or retry custody.
    fn check_document(&self, target: ProbeTarget) -> Result<(), ReadinessFault> {
        let current = self.default_context == Some(target.context)
            && self
                .root
                .is_some_and(|root| root.id == target.frame && root.loader == target.loader);
        if current { Ok(()) } else { Err(ReadinessFault::StaleProbe) }
    }
    fn bind_default_context(&mut self) -> Result<(), ReadinessFault> {
        let Some(root) = self.root else { return Ok(()) };
        let mut found = None;
        let mut failure = None;
        for context in self
            .tables
            .contexts
            .iter()
            .filter(|context| context.default && context.frame == root.id)
        {
            if context.origin != Some(self.origin) {
                failure = Some(ReadinessFault::WrongOrigin);
                break;
            }
            if found.replace(context.id).is_some() {
                failure = Some(ReadinessFault::ContextLost);
                break;
            }
        }
        if let Some(failure) = failure {
            return self.reject(failure);
        }
        self.default_context = found;
        Ok(())
    }
}

#[cfg(test)]
#[path = "main_readiness_tests.rs"]
mod tests;
