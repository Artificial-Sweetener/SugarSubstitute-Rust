// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine overlapped pipes, bootstrap sections and credential inspection to native ownership.
use crate::{
    lifecycle_pipe::*,
    native::{NativeObservationError, Signal},
    native_deadlines::Role,
};
use std::{os::windows::io::IntoRawHandle, sync::Arc};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    lifecycle::{ProcessKey, RunKey},
    recovery_supervisor::RecoveryGrantKey,
};
use windows::{
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Pipes::*, Threading::*},
    },
    core::PCWSTR,
};
#[path = "bootstrap.rs"]
pub(super) mod bootstrap;
#[path = "credentials.rs"]
mod credentials;
#[path = "peer.rs"]
pub(in crate::windows_lifecycle_pipe) mod peer;
#[path = "prepare.rs"]
mod prepare;
#[path = "resource_binding.rs"]
mod resource_binding;
use crate::native_pipe::{
    handles::{Handle, adopt, last_error},
    overlapped, security,
};
// Message-mode setup needs attribute-write access; generic write would also
// grant FILE_CREATE_PIPE_INSTANCE through the append-data bit.
pub(super) const CLIENT_ACCESS: u32 = FILE_READ_DATA.0
    | FILE_WRITE_DATA.0
    | FILE_READ_ATTRIBUTES.0
    | FILE_WRITE_ATTRIBUTES.0
    | SYNCHRONIZE.0;
pub(crate) struct Lease {
    pub(in crate::windows_lifecycle_pipe) recovery_pair:
        Option<Box<super::recovery_pair::NativeRecoveryPair>>,
    pub(in crate::windows_lifecycle_pipe) captured_parent:
        Option<super::captured::parent::ParentSource>,
    pub(in crate::windows_lifecycle_pipe) captured_client: bool,
    pub(in crate::windows_lifecycle_pipe) captured_facts:
        Option<super::captured::codec::CapturedFacts>,
    pub(in crate::windows_lifecycle_pipe) captured_until: Option<Moment>,
    #[cfg(feature = "k1-native-qualification")]
    pub(in crate::windows_lifecycle_pipe) captured_native_scope:
        Option<sugarsubstitute_contracts::execution::ScopeId>,
    pub(in crate::windows_lifecycle_pipe) captured_ready:
        Option<super::captured::child::CapturedInput>,
    pub(in crate::windows_lifecycle_pipe) capture_processes: [Handle; 2],
    pub(in crate::windows_lifecycle_pipe) capture_tokens: [Handle; 2],
    symbol: Option<crate::SymbolTransportReservation>,
    server: bool,
    binding: Option<PeerBinding>,
    pub(in crate::windows_lifecycle_pipe) original: Option<LifecyclePipeDeadline>,
    pub(in crate::windows_lifecycle_pipe) expected: bootstrap::Kind,
    pub(in crate::windows_lifecycle_pipe) data: Option<bootstrap::Data>,
    pub(in crate::windows_lifecycle_pipe) input: Handle,
    process: Option<ProcessKey>,
    pipe: Handle,
    pub(in crate::windows_lifecycle_pipe) section: bootstrap::Section,
    pub(in crate::windows_lifecycle_pipe) own_token: Handle,
    peer_token: Handle,
    observed_token: Handle,
    message_token: Handle,
    observed_process: Handle,
    pub(in crate::windows_lifecycle_pipe) identity: Option<security::Identity>,
    operations: [Option<overlapped::Operation>; 3],
    read_completed: bool,
}
impl Lease {
    fn new(
        server: bool,
        binding: Option<PeerBinding>,
        original: Option<LifecyclePipeDeadline>,
        expected: bootstrap::Kind,
    ) -> Self {
        Self {
            recovery_pair: None,
            captured_parent: None,
            captured_client: false,
            captured_facts: None,
            captured_until: None,
            #[cfg(feature = "k1-native-qualification")]
            captured_native_scope: None,
            captured_ready: None,
            capture_processes: std::array::from_fn(|_| Handle::new()),
            capture_tokens: std::array::from_fn(|_| Handle::new()),
            symbol: None,
            server,
            binding,
            expected,
            original,
            data: None,
            input: Handle::new(),
            process: None,
            pipe: Handle::new(),
            section: bootstrap::Section::new(),
            own_token: Handle::new(),
            peer_token: Handle::new(),
            observed_token: Handle::new(),
            message_token: Handle::new(),
            observed_process: Handle::new(),
            identity: None,
            operations: std::array::from_fn(|_| None),
            read_completed: false,
        }
    }
    pub(super) fn server(run: RunKey) -> Self {
        Self::new(true, Some(PeerBinding::Desktop(run)), None, bootstrap::Kind::Desktop)
    }
    /// Count this concrete native lease, separately boxed output owners and worker metadata.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>()
            + 11 * (Handle::storage_bytes() - size_of::<Handle>())
            + bootstrap::Section::storage_bytes()
            - size_of::<bootstrap::Section>()
            + 3 * overlapped::Operation::storage_bytes()
            + crate::lifecycle_pipe::worker::ActiveState::storage_bytes()
            + 3 * (size_of::<Signal>() + 2 * size_of::<std::sync::atomic::AtomicUsize>())
            + size_of::<security::Descriptor>()
            + size_of::<[u32; 128]>()
            + size_of::<bootstrap::Data>()
            + super::recovery_pair::NativeRecoveryPair::storage_bytes()
    }
    /// Install the original captured map/control claim before any mapping effect.
    pub(in crate::windows_lifecycle_pipe) fn install_captured_mapping(
        &mut self,
        claim: sugarsubstitute_contracts::execution::MemoryClaim,
        signal: Arc<Signal>,
        extent: usize,
    ) -> Result<(), LifecyclePipeFailure> {
        self.section.install_mapping(claim, signal, extent, self.expected)
    }
    pub(super) fn client(original: LifecyclePipeDeadline) -> Self {
        Self::new(false, None, Some(original), bootstrap::Kind::Desktop)
    }
    /// Prepare one purpose-typed server endpoint for its exact binding.
    pub(crate) fn purpose_server(key: crate::purpose_pipe::PurposeKey) -> Self {
        Self::new(
            true,
            Some(PeerBinding::Purpose(key)),
            None,
            bootstrap::Kind::Purpose(key.purpose()),
        )
    }
    /// Decode only a bootstrap of the expected purpose before adopting any native peer operation.
    pub(crate) fn purpose_client(
        purpose: crate::purpose_pipe::Purpose,
        original: LifecyclePipeDeadline,
        symbol: Option<crate::SymbolTransportReservation>,
    ) -> Self {
        let mut lease = Self::new(false, None, Some(original), bootstrap::Kind::Purpose(purpose));
        lease.symbol = symbol;
        lease.bind_original_context();
        lease
    }
    /// Prepare the distinct custody endpoint with the existing pipe/security owner.
    pub(crate) fn backend_custody_server(key: BackendCustodyKey) -> Self {
        Self::new(
            true,
            Some(PeerBinding::BackendCustody(key)),
            None,
            bootstrap::Kind::BackendCustody,
        )
    }
    /// Decode only a custody bootstrap before adopting any native peer operation.
    pub(crate) fn backend_custody_client(original: LifecyclePipeDeadline) -> Self {
        Self::new(false, None, Some(original), bootstrap::Kind::BackendCustody)
    }
    /// Construct the dedicated Root successor endpoint without native effects.
    pub(crate) fn recovery_server(key: RecoveryGrantKey) -> Self {
        Self::new(true, Some(PeerBinding::Recovery(key)), None, bootstrap::Kind::Recovery)
    }
    /// Decode only the private recovery bootstrap of the dedicated invocation.
    pub(crate) fn recovery_client(original: LifecyclePipeDeadline) -> Self {
        let mut owner = Self::new(false, None, Some(original), bootstrap::Kind::Recovery);
        owner.recovery_pair = Some(Box::new(super::recovery_pair::NativeRecoveryPair::new()));
        owner
    }
    fn operation(
        &mut self,
        role: Role,
    ) -> Result<&mut overlapped::Operation, LifecyclePipeFailure> {
        self.operations[role.index()].as_mut().ok_or_else(|| failure("pipe_operation_owner"))
    }
}
impl PipeLease for Lease {
    fn adopt(&mut self, input: PeerInput) -> Result<(), (Box<LifecyclePipeFailure>, PeerInput)> {
        if let PeerInput::RecoveryInherited { section, desktop_section } = &input
            && let Err(cause) = crate::windows_recovery_supervisor::inheritance::validate_pair_slots(
                *section as u64,
                *desktop_section as u64,
            )
        {
            return Err((Box::new(error("recovery_pair_input_slots", cause)), input));
        }
        if let Err(cause) = self.input.reserve_custody("pipe_adopt_input_accounting") {
            return Err((Box::new(cause.into()), input));
        }
        if matches!(&input, PeerInput::RecoveryInherited { .. }) {
            let Some(pair) = self.recovery_pair.as_mut() else {
                return Err((Box::new(failure("recovery_pair_wrong_purpose")), input));
            };
            if let Err(cause) = pair.reserve_input() {
                return Err((Box::new(cause), input));
            }
        }
        match input {
            PeerInput::RecoveryInherited { section, desktop_section } => {
                // Both actual outputs are adopted before later schema/native checks.
                self.input.adopt_partial(HANDLE(section as *mut _));
                if let Some(pair) = &mut self.recovery_pair {
                    pair.adopt(desktop_section);
                }
            }
            PeerInput::Owned { handle, process } => {
                self.process = process;
                self.input.adopt_partial(HANDLE(handle.into_raw_handle()));
            }
            PeerInput::Inherited { section } => self.input.adopt_partial(HANDLE(section as *mut _)),
            #[cfg(test)]
            PeerInput::Simulated => {}
        }
        Ok(())
    }
    fn prepare(
        &mut self,
        signals: [Arc<Signal>; 3],
        context: &JobContext,
        handshake: Moment,
    ) -> Result<usize, LifecyclePipeFailure> {
        self.bind_effect_context(context);
        for (slot, signal) in self.operations.iter_mut().zip(signals) {
            let mut operation = overlapped::Operation::new(signal);
            if let Some(symbol) = &self.symbol {
                operation.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
                    symbol.handles.clone(),
                    crate::native_resources::EffectContext::new(symbol).for_child(context),
                ));
            }
            *slot = Some(operation);
        }
        if self.server {
            self.prepare_server(context, handshake)
        } else {
            self.prepare_client(context)
        }
    }
    fn binding(&self) -> Option<PeerBinding> {
        self.binding
    }
    fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.data.as_ref().map(|data| data.deadline)
    }
    fn capability(&self) -> Option<[u8; 32]> {
        if self.server { None } else { self.data.as_ref().map(|data| data.capability) }
    }
    fn connect(&mut self, issue: Issue<'_>) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        if !self.server {
            return match issue.execute(|| self.claim_client_open())? {
                Dispatch::Issued(stage) => {
                    stage.open()?;
                    self.configure_client()?;
                    self.verified_kernel()?;
                    Ok(Dispatch::Issued(Progress::Complete(0)))
                }
                Dispatch::Revoked => Ok(Dispatch::Revoked),
                Dispatch::Stopped => Ok(Dispatch::Stopped),
                Dispatch::Expired => Ok(Dispatch::Expired),
            };
        }
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Handshake)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: retained prepared OVERLAPPED remains pinned through checked native retirement.
            let result = unsafe { ConnectNamedPipe(pipe, Some(operation.pointer())) };
            operation.accept(result, true).map_err(LifecyclePipeFailure::from)
        })? {
            Dispatch::Issued(accepted) => {
                Ok(Dispatch::Issued(operation.initial_progress(accepted, pipe)?))
            }
            Dispatch::Revoked => Ok(Dispatch::Revoked),
            Dispatch::Stopped => Ok(Dispatch::Stopped),
            Dispatch::Expired => Ok(Dispatch::Expired),
        }
    }
    fn read(
        &mut self,
        bytes: &mut [u8],
        issue: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Read)?;
        operation.prepare()?;
        let result = match issue.execute(|| {
            // SAFETY: exact charged buffer and pinned operation remain outside cancellable results.
            let result = unsafe { ReadFile(pipe, Some(bytes), None, Some(operation.pointer())) };
            operation.accept(result, false).map_err(LifecyclePipeFailure::from)
        })? {
            Dispatch::Issued(accepted) => {
                Dispatch::Issued(operation.initial_progress(accepted, pipe)?)
            }
            Dispatch::Revoked => Dispatch::Revoked,
            Dispatch::Stopped => Dispatch::Stopped,
            Dispatch::Expired => Dispatch::Expired,
        };
        if matches!(result, Dispatch::Issued(Progress::Complete(_))) {
            self.read_completed = true;
        }
        Ok(result)
    }
    fn write(
        &mut self,
        bytes: &[u8],
        issue: Issue<'_>,
    ) -> Result<Dispatch<Progress>, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let operation = self.operation(Role::Write)?;
        operation.prepare()?;
        match issue.execute(|| {
            // SAFETY: exact immutable buffer and prepared OVERLAPPED stay retained through settlement.
            let result = unsafe { WriteFile(pipe, Some(bytes), None, Some(operation.pointer())) };
            operation.accept(result, false).map_err(LifecyclePipeFailure::from)
        })? {
            Dispatch::Issued(accepted) => {
                Ok(Dispatch::Issued(operation.initial_progress(accepted, pipe)?))
            }
            Dispatch::Revoked => Ok(Dispatch::Revoked),
            Dispatch::Stopped => Ok(Dispatch::Stopped),
            Dispatch::Expired => Ok(Dispatch::Expired),
        }
    }
    fn poll(&mut self, role: Role) -> Result<Progress, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let result = self.operation(role)?.poll(pipe)?;
        if role == Role::Read && matches!(result, Progress::Complete(_)) {
            self.read_completed = true;
        }
        Ok(result)
    }
    fn quiesce(&mut self, role: Role) -> Result<(), LifecyclePipeFailure> {
        self.operation(role)?.quiesce().map_err(LifecyclePipeFailure::from)
    }
    fn verify_message(&mut self) -> Result<(), LifecyclePipeFailure> {
        if !self.read_completed {
            return Err(failure("pipe_verify_before_read"));
        }
        self.verified_kernel()?;
        if !self.server {
            return Ok(());
        }
        let identity = self.identity.ok_or_else(|| failure("pipe_identity_owner"))?;
        // The previous token remains owned until this checked close succeeds.
        self.message_token.close("pipe_close_previous_message_token")?;
        let actual = security::message_identity(self.pipe.raw(), &mut self.message_token)?;
        peer::identity(actual, identity, "pipe_message_identity_mismatch")
    }
    fn authenticate(&mut self, candidate: &[u8; 32]) -> Result<VerifiedPeer, LifecyclePipeFailure> {
        let peer = self.verified_kernel()?;
        let data = self.data.as_ref().ok_or_else(|| failure("pipe_bootstrap_owner"))?;
        peer::capability(candidate, &data.capability)?;
        if self.captured_client {
            self.authenticate_captured_child(peer)?;
        }
        if let Some(pair) = &mut self.recovery_pair {
            pair.authenticate(peer)?;
        }
        Ok(peer)
    }
    fn inherited_capture_processes(&self) -> Option<[usize; 2]> {
        self.captured_parent.as_ref().map(|parent| parent.processes)
    }
    fn take_captured_startup(&mut self) -> Option<super::captured::child::CapturedInput> {
        self.captured_ready.take()
    }
    fn take_recovery_desktop_bootstrap(
        &mut self,
    ) -> Option<crate::recovery_supervisor::root_desktop::RootDesktopBootstrapLoan> {
        self.recovery_pair.as_mut().and_then(|pair| pair.take())
    }
    fn take_completed_write(&mut self) -> Option<u32> {
        self.operations[Role::Write.index()]
            .as_mut()
            .and_then(overlapped::Operation::take_completed)
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, LifecyclePipeFailure> {
        let pipe = self.pipe.raw();
        let mut settled = true;
        let mut failure: Option<LifecyclePipeFailure> = None;
        for operation in self.operations.iter_mut().flatten() {
            match operation.retire(pipe) {
                Ok(retired) => settled &= retired,
                Err(error) => {
                    settled = false;
                    if let Some(first) = &mut failure {
                        first.identity_unrestored |= error.identity_unrestored;
                        first.cause.retain_cleanup(error.cause);
                    } else {
                        failure = Some(error.into());
                    }
                }
            }
        }
        if let Some(failure) = failure {
            return Err(failure);
        }
        if !settled {
            return Ok(false);
        }
        self.pipe.close("pipe_close_connection")?;
        if let Some(pair) = &mut self.recovery_pair
            && !pair.retire()?
        {
            return Ok(false);
        }
        // Untaken authenticated loans return outside all internal owner locks.
        drop(self.captured_ready.take());
        if !self.section.retire(pinned)? {
            return Ok(false);
        }
        for handle in [
            &mut self.message_token,
            &mut self.peer_token,
            &mut self.observed_token,
            &mut self.own_token,
            &mut self.observed_process,
            &mut self.input,
        ] {
            handle.close("pipe_close_security_handle")?;
        }
        for handle in self.capture_tokens.iter_mut().chain(self.capture_processes.iter_mut()) {
            handle.close("captured_close_inherited_security_handle")?;
        }
        if let Some(data) = &mut self.data {
            data.capability.fill(0);
        }
        Ok(true)
    }
}

pub(in crate::windows_lifecycle_pipe) fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> LifecyclePipeFailure {
    LifecyclePipeFailure::native(NativeObservationError::new(stage, cause))
}
