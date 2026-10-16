// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind expected native sources and listener authority independently of response metadata.
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::{configuration::LaunchConfiguration, lifecycle::ResourceKey};
use sugarsubstitute_supervision::{
    backend_listener::{AttachedListenerLease, BackendPeerIdentity, ManagedListenerLease},
    installed_frontend::InstalledFrontendLease,
};

pub(super) enum Listener {
    Managed(Box<ManagedListenerLease>, LaunchConfiguration),
    Attached(AttachedListenerLease, u16),
}
/// Own explicit expected installed files and one exact managed or attached loopback connection.
pub struct ReadinessProbeInput {
    pub(super) source: InstalledFrontendLease,
    pub(super) listener: Listener,
}
impl ReadinessProbeInput {
    /// Compare the retained effective target without granting native authority.
    pub(crate) fn matches_target(
        &self,
        target: &sugarsubstitute_contracts::lifecycle::BackendTarget,
    ) -> bool {
        use sugarsubstitute_contracts::lifecycle::BackendTarget;
        match (&self.listener, target) {
            (Listener::Managed(_, retained), BackendTarget::Managed(expected)) => {
                retained == expected
            }
            (Listener::Attached(_, port), BackendTarget::Attached(origin)) => {
                origin.host() == sugarsubstitute_contracts::configuration::LoopbackHost::Ipv4
                    && *port == origin.port().get()
            }
            _ => false,
        }
    }
    /// Bind a managed lease and the complete actual effective launch configuration.
    /// # Errors
    /// Rejects retired installed sources and configurations whose retained storage exceeds the bound.
    pub fn managed(
        source: InstalledFrontendLease,
        listener: ManagedListenerLease,
        configuration: LaunchConfiguration,
    ) -> Result<Self, ReadinessTransportError> {
        source.validate_live().map_err(|e| ReadinessTransportError::new("expected_source", e))?;
        if !backend_script_matches(
            source.manifest().backend_script(),
            &configuration.root().join("main.py"),
        ) {
            return Err(ReadinessTransportError::invalid("managed_backend_script_binding"));
        }
        let bytes = configuration.root().as_os_str().len() * 3
            + configuration.interpreter().as_os_str().len() * 3
            + configuration.arguments().iter().map(String::len).sum::<usize>();
        if bytes > 512 * 1024 {
            return Err(ReadinessTransportError::invalid("managed_configuration_storage"));
        }
        let configuration = LaunchConfiguration::new(
            configuration.root().to_path_buf(),
            configuration.interpreter().to_path_buf(),
            configuration.arguments().iter().map(|arg| arg.as_str().to_owned()).collect(),
            configuration.port().get(),
        )
        .map_err(|e| ReadinessTransportError::new("managed_configuration", e))?;
        Ok(Self { source, listener: Listener::Managed(Box::new(listener), configuration) })
    }
    /// Bind an explicit attached port and independent installed-source proof.
    /// The attached lease has read-only process identity access and no stop authority.
    /// # Errors
    /// Rejects zero ports or a retired installed source.
    pub fn attached(
        source: InstalledFrontendLease,
        port: u16,
    ) -> Result<Self, ReadinessTransportError> {
        source.validate_live().map_err(|e| ReadinessTransportError::new("expected_source", e))?;
        if port == 0 {
            return Err(ReadinessTransportError::invalid("attached_port"));
        }
        Ok(Self { source, listener: Listener::Attached(AttachedListenerLease::new(), port) })
    }
    pub(super) fn port(&self) -> u16 {
        match &self.listener {
            Listener::Managed(_, configuration) => configuration.port().get(),
            Listener::Attached(_, port) => *port,
        }
    }
}
/// Accept one exact DOS spelling with an optional verbatim prefix; resolve no aliases.
pub(super) fn backend_script_matches(
    canonical: &std::path::Path,
    launch: &std::path::Path,
) -> bool {
    let prefix = b"\\\\?\\";
    let canonical = canonical.as_os_str().as_encoded_bytes();
    let launch = launch.as_os_str().as_encoded_bytes();
    let launch = launch.strip_prefix(prefix).unwrap_or(launch);
    launch.len() > 3
        && launch[0].is_ascii_alphabetic()
        && launch[1..3] == *b":\\"
        && !launch.contains(&b'/')
        && launch[3..]
            .split(|byte| *byte == b'\\')
            .all(|part| !part.is_empty() && part != b"." && part != b"..")
        && canonical.strip_prefix(prefix) == Some(launch)
}
/// Observe verified HTTP/file/peer facts; browser execution remains a separate qualification.
/// Private construction prevents metadata-only or synthetic HTTP-success promotion.
pub struct VerifiedReadiness {
    pub(super) input: Arc<ReadinessProbeInput>,
    pub(super) peer: BackendPeerIdentity,
}
impl VerifiedReadiness {
    /// Return exact managed resource generation, or explicit attachment.
    pub fn managed_resource(&self) -> Option<ResourceKey> {
        match &self.input.listener {
            Listener::Managed(lease, _) => Some(lease.resource()),
            Listener::Attached(_, _) => None,
        }
    }
    /// Return actual kernel PID observed at both ends of every served request.
    pub const fn process_id(&self) -> u32 {
        self.peer.process_id()
    }
    /// Return actual kernel birth in 100ns units since January 1, 1601 UTC.
    pub const fn creation_time_100ns(&self) -> u64 {
        self.peer.creation_time_100ns()
    }
    /// Borrow independently observed expected installed versions and asset digests.
    pub fn source(&self) -> &InstalledFrontendLease {
        &self.input.source
    }
    /// Recheck installed custody and the managed stop fence before application publication.
    /// # Errors
    /// Rejects explicit source closure or managed family shutdown since delivery.
    pub fn validate_current(&self) -> Result<(), ReadinessTransportError> {
        self.input
            .source
            .validate_live()
            .map_err(|e| ReadinessTransportError::new("expected_source_retired", e))?;
        if let Listener::Managed(lease, _) = &self.input.listener
            && !lease.is_live()
        {
            return Err(ReadinessTransportError::invalid("managed_family_retiring"));
        } else if let Listener::Attached(lease, _) = &self.input.listener
            && !lease.is_live()
        {
            return Err(ReadinessTransportError::invalid("attached_listener_retiring"));
        }
        Ok(())
    }
}
/// Classify retry eligibility at the original native failure boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeRetryDisposition {
    /// A native request failed to establish or retain its first unverified connection.
    TransientConnectionUnavailable,
    /// The execution owner delivered a genuine finite probe timeout, with original custody retained.
    /// Only an admitted managed-live campaign may retry after actual cleanup.
    ProbeIntervalElapsed,
    /// Trust, shape, identity, other deadlines, cancellation, admission or cleanup failed.
    Terminal,
}

/// Preserve an original boundary cause while redacting private response/path data.
pub struct ReadinessTransportError {
    stage: &'static str,
    cause: Box<dyn Error + Send + Sync>,
    disposition: ProbeRetryDisposition,
    body: Option<super::memory::BodyClaim>,
}
impl ReadinessTransportError {
    pub(super) fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self {
            stage,
            cause: Box::new(cause),
            disposition: ProbeRetryDisposition::Terminal,
            body: None,
        }
    }
    /// Preserve a typed native first-connection failure eligible only for campaign retry.
    pub(super) fn unavailable(
        stage: &'static str,
        cause: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            stage,
            cause: Box::new(cause),
            disposition: ProbeRetryDisposition::TransientConnectionUnavailable,
            body: None,
        }
    }
    /// Retain actual finite-attempt expiry without treating it as connection or readiness proof.
    pub(super) fn elapsed(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self {
            stage,
            cause: Box::new(cause),
            disposition: ProbeRetryDisposition::ProbeIntervalElapsed,
            body: None,
        }
    }
    pub(super) fn invalid(stage: &'static str) -> Self {
        Self::new(stage, std::io::Error::other("Readiness transport rejected"))
    }
    pub(super) fn retain_body(mut self, body: super::memory::BodyClaim) -> Self {
        if self.body.is_none() {
            self.body = Some(body);
        }
        self
    }
    /// Borrow the stable operation label.
    pub const fn stage(&self) -> &'static str {
        self.stage
    }
    /// Return the failure owner's stored decision without inspecting text or the cause chain.
    pub const fn retry_disposition(&self) -> ProbeRetryDisposition {
        self.disposition
    }
}
impl fmt::Display for ReadinessTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ComfyUI readiness transport failure: {}", self.stage)
    }
}
impl fmt::Debug for ReadinessTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl Error for ReadinessTransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
