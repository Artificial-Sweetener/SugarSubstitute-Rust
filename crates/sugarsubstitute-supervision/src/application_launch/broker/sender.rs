// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Engine-side once-only publication through its inherited native creator channel.
use super::*;
use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{
    application_launch::updater::{
        ACTIVATION_ARGUMENT,
        activation::{ActivationRequest, MAX_REQUEST_BYTES},
    },
    configuration::LaunchConfiguration,
};
use sugarsubstitute_execution::activation_channel::{
    ChannelEndpoint, ChannelProgress, InheritedChannel, capture_inherited,
};

/// Retain early inherited custody, publication and actual reply independently of worker results.
/// This owner never creates an application. Its caller retains the offered `ApplicationLaunch`
/// until reply observation and source retirement, or the dedicated engine's final process exit.
pub struct ActivationSender {
    root: PathBuf,
    identity: [u8; 32],
    state: Mutex<State>,
}
/// A channel is admitted once and can authorize only one application request.
struct State {
    inherited: Option<InheritedChannel>,
    endpoint: Option<ChannelEndpoint>,
    offer: Option<LaunchOffer>,
    receiving: bool,
    outcome: Option<CreationOutcome>,
}
impl ActivationSender {
    /// Installation path supplied at entry and subsequently matched to authenticated engine state.
    pub fn installation(&self) -> &Path {
        &self.root
    }

    /// Publish one already-prepared launch, retaining it before issuing any native I/O.
    /// Reserve channel plus transfer bytes on an admitted worker with the startup cutoff.
    /// # Errors
    /// Rejects replay, invalid custody, execution cancellation and native channel errors.
    pub fn offer(&self, launch: &ApplicationLaunch, context: &JobContext) -> io::Result<()> {
        let mut state = crate::native_claims::lock(&self.state);
        if state.endpoint.is_some() || state.offer.is_some() {
            return Err(io::Error::other("Activation publication already admitted"));
        }
        let captured =
            state.inherited.take().ok_or_else(|| io::Error::other("Activation channel absent"))?;
        state.endpoint = Some(captured.admit(context).map_err(io::Error::other)?);
        state.offer = Some(launch.offer(self.identity, context).map_err(io::Error::other)?);
        let offer =
            state.offer.as_ref().ok_or_else(|| io::Error::other("Activation offer absent"))?;
        state
            .endpoint
            .as_ref()
            .ok_or_else(|| io::Error::other("Activation endpoint absent"))?
            .send(offer.frame(context).map_err(io::Error::other)?, context)
            .map_err(io::Error::other)
    }

    /// Observe finite transport progress; only a correlated creator reply establishes disposition.
    /// # Errors
    /// Preserves original publication, native I/O, protocol and execution failures.
    pub fn poll(&self, context: &JobContext) -> io::Result<Option<CreationOutcome>> {
        let mut state = crate::native_claims::lock(&self.state);
        if let Some(outcome) = state.outcome {
            return Ok(Some(outcome));
        }
        let endpoint = state
            .endpoint
            .as_ref()
            .ok_or_else(|| io::Error::other("Activation endpoint absent"))?;
        match endpoint.poll(context).map_err(io::Error::other)? {
            ChannelProgress::Pending => Ok(None),
            ChannelProgress::Sent if !state.receiving => {
                endpoint.receive(context).map_err(io::Error::other)?;
                state.receiving = true;
                Ok(None)
            }
            ChannelProgress::Received(frame) if state.receiving => {
                let outcome = state
                    .offer
                    .as_ref()
                    .ok_or_else(|| io::Error::other("Activation offer absent"))?
                    .observe_result(frame.bytes(), context)
                    .map_err(io::Error::other)?;
                state.outcome = Some(outcome);
                Ok(Some(outcome))
            }
            _ => Err(io::Error::other("Unexpected activation channel direction")),
        }
    }

    /// Cancel and settle native I/O before the caller retires source launch custody.
    /// Uses independent cleanup admission; pending or failed cleanup retains its actual outputs.
    /// # Errors
    /// Preserves checked native cleanup errors and the original cleanup cutoff.
    pub fn retire(&self, context: &JobContext) -> io::Result<bool> {
        let mut state = crate::native_claims::lock(&self.state);
        if let Some(endpoint) = &state.endpoint
            && !endpoint.retire(context).map_err(io::Error::other)?
        {
            return Ok(false);
        }
        state.endpoint = None;
        state.inherited = None;
        state.offer = None;
        Ok(true)
    }
}

/// Capture the activation channel once before diagnostics, execution or other handle owners.
/// Ordinary updater roles leave this capture empty. Unsupported activation versions fail closed.
/// # Safety
/// Call exactly once at initial process entry. The supplied inherited slot has no other owner
/// and cannot be closed or reused concurrently through native validation and adoption.
/// # Errors
/// Rejects malformed arguments, unsupported versions, invalid paths and non-channel handles.
pub unsafe fn capture_activation(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<ActivationSender>> {
    let Some((root, request)) = parse(arguments)? else { return Ok(None) };
    let raw = usize::try_from(request.channel()).map_err(io::Error::other)?;
    // SAFETY: initial-entry caller guarantees once-only ownership and excludes slot reuse.
    let inherited = unsafe { capture_inherited(raw) }.map_err(io::Error::other)?;
    Ok(Some(ActivationSender {
        root,
        identity: request.identity(),
        state: Mutex::new(State {
            inherited: Some(inherited),
            endpoint: None,
            offer: None,
            receiving: false,
            outcome: None,
        }),
    }))
}

/// Parse all bounded input before interpreting a handle value as native ownership.
fn parse(
    arguments: impl IntoIterator<Item = OsString>,
) -> io::Result<Option<(PathBuf, ActivationRequest)>> {
    let invalid = || io::Error::other("Invalid startup activation entry");
    let mut arguments = arguments.into_iter();
    let Some(prefix) = arguments.next() else { return Ok(None) };
    if prefix != ACTIVATION_ARGUMENT {
        if prefix.to_str().is_some_and(|value| value.starts_with("--internal-update-activation-")) {
            return Err(invalid());
        }
        return Ok(None);
    }
    let root = PathBuf::from(arguments.next().ok_or_else(invalid)?);
    LaunchConfiguration::validate_path(&root).map_err(io::Error::other)?;
    if root.capacity() > 256 * 1024 {
        return Err(invalid());
    }
    let encoded = arguments.next().ok_or_else(invalid)?;
    if encoded.len() > MAX_REQUEST_BYTES || arguments.next().is_some() {
        return Err(invalid());
    }
    let encoded = encoded.into_string().map_err(|_| invalid())?;
    let request = ActivationRequest::decode(encoded.as_bytes()).map_err(io::Error::other)?;
    Ok(Some((root, request)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_capture_parses_every_argument_before_adopting_native_custody()
    -> Result<(), Box<dyn std::error::Error>> {
        let encoded = ActivationRequest::new(12, [31; 32])?.encode()?;
        let arguments =
            [ACTIVATION_ARGUMENT.into(), OsString::from(r"E:\native installation"), encoded.into()];
        let (root, request) = parse(arguments.clone())?.ok_or("No activation")?;
        assert_eq!(root, Path::new(r"E:\native installation"));
        assert_eq!(request.channel(), 12);
        assert!(parse(arguments.into_iter().chain([OsString::from("extra")])).is_err());
        assert!(parse([OsString::from("--internal-update-activation-v1")]).is_err());
        assert!(parse([OsString::from(ACTIVATION_ARGUMENT)]).is_err());
        assert!(parse([OsString::from("--internal-update-preparation")])?.is_none());
        Ok(())
    }
}
