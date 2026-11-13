// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize unread-response discard and genuine retained peer verification before checked EOF.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_contracts::{
    execution::{ExecutionPort, JobClass, JobOutcome, MemoryClaim},
    lifecycle::NamespaceId,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Pins {
    server: Handle,
    client: Handle,
    token: Handle,
    image: Handle,
    peer: PeerPin,
    charge: Option<MemoryClaim>,
}
struct Capsule(Option<Pins>);
impl Drop for Capsule {
    fn drop(&mut self) {
        if let Some(pins) = self.0.take()
            && (pins.server.value() != 0
                || pins.client.value() != 0
                || pins.token.value() != 0
                || pins.image.value() != 0
                || pins.peer.process.value() != 0
                || pins.peer.token.value() != 0
                || pins.peer.image.value() != 0
                || pins.peer.message_token.value() != 0)
        {
            // Keep all actual output cells and the original admitted claim together on uncertainty.
            std::mem::forget(pins);
        }
    }
}
impl Pins {
    fn retire(&mut self) -> Result<(), EntryTransportError> {
        let mut cause = None;
        if let Err(error) = self.peer.retire() {
            retain(&mut cause, error);
        }
        for handle in [&mut self.client, &mut self.server, &mut self.image, &mut self.token] {
            if let Err(error) = handle.close("kernel_fixture_checked_close") {
                retain(&mut cause, error.into());
            }
        }
        cause.map_or(Ok(()), Err)
    }
    fn open(&mut self, own: Identity) -> Result<(), EntryTransportError> {
        let name = format!(
            "\\\\.\\pipe\\SugarSubstitute.QueryFixture.{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::AcqRel)
        );
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut descriptor =
            security::Descriptor::new(own, security::DescriptorPurpose::CoordinatorServer)?;
        let attributes = descriptor.attributes();
        // SAFETY: unique local endpoint, fixed small buffers and synchronous operations owned in this job.
        let raw = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                1024,
                1024,
                0,
                Some(&attributes),
            )
        };
        adopt(&mut self.server, raw, "kernel_fixture_create_server")?;
        // SAFETY: the same unique local existing endpoint; adopt before any following query.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| error("kernel_fixture_open_client", e))?;
        adopt(&mut self.client, raw, "kernel_fixture_open_client")?;
        // SAFETY: already-opened actual client and synchronous server, no OVERLAPPED pointer.
        if let Err(e) = unsafe { ConnectNamedPipe(self.server.raw(), None) }
            && e.code() != windows::core::HRESULT::from_win32(ERROR_PIPE_CONNECTED.0)
        {
            return Err(error("kernel_fixture_connect", e));
        }
        Ok(())
    }
    fn expected(&mut self) -> Result<(EntryNativeIdentityFacts, Identity), EntryTransportError> {
        // SAFETY: Windows-owned pseudo process handle is only queried, never adopted/closed.
        let current = unsafe { GetCurrentProcess() };
        let own = security::process_identity(current, &mut self.token)?;
        let image = image_identity(current, &mut self.image)?;
        let facts = EntryNativeIdentityFacts::new(
            NamespaceId::new([19; 32]),
            own.account_sid(),
            own.session,
            image,
        )
        .map_err(|_| EntryTransportError::Authentication)?;
        Ok((facts, own))
    }
    fn write_reply(&mut self) -> Result<(), EntryTransportError> {
        let mut count = 0;
        // SAFETY: actual synchronous server and fixed local response bytes live for the entire call.
        unsafe { WriteFile(self.server.raw(), Some(b"retained reply"), Some(&mut count), None) }
            .map_err(|e| error("kernel_fixture_write", e))?;
        if count != 14 {
            return Err(EntryTransportError::Malformed);
        }
        Ok(())
    }
}
fn qualify(disconnect: bool) -> TestResult {
    let rig = Rig::new()?;
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(1, JobClass::InfrastructureLifecycle, 8192)?,
        move |context| -> Result<
            (u32, Option<windows::core::HRESULT>, MemoryClaim),
            EntryTransportError,
        > {
            let charge =
                context.memory().claim(8192).map_err(|e| error("kernel_fixture_claim", e))?;
            let mut capsule = Capsule(Some(Pins {
                server: Handle::new(),
                client: Handle::new(),
                token: Handle::new(),
                image: Handle::new(),
                peer: PeerPin::new(),
                charge: Some(charge),
            }));
            let pins = capsule.0.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
            let observed =
                (|| -> Result<(u32, Option<windows::core::HRESULT>), EntryTransportError> {
                    let (expected, own) = pins.expected()?;
                    pins.open(own)?;
                    let first =
                        pins.peer.verify(pins.client.raw(), false, &expected, own, false)?;
                    pins.write_reply()?;
                    if disconnect {
                        // SAFETY: emulate the exact old server retirement after a completed final write.
                        unsafe { DisconnectNamedPipe(pins.server.raw()) }
                            .map_err(|e| error("kernel_fixture_premature_disconnect", e))?;
                        let mut pid = 0;
                        // SAFETY: original client endpoint remains owned after server disconnect.
                        let cause =
                            unsafe { GetNamedPipeServerProcessId(pins.client.raw(), &mut pid) }
                                .err()
                                .ok_or(EntryTransportError::InvalidPhase)?;
                        let mut bytes = [0; 14];
                        let mut count = 0;
                        // SAFETY: fixed owned synchronous buffer; disconnected server cannot block this read.
                        let read = unsafe {
                            ReadFile(pins.client.raw(), Some(&mut bytes), Some(&mut count), None)
                        };
                        if read.is_ok() && count != 0 {
                            return Err(EntryTransportError::Malformed);
                        }
                        return Ok((first.pid, Some(cause.code())));
                    }
                    let mut bytes = [0; 14];
                    let mut count = 0;
                    // SAFETY: the entire fixed message was actually written before this bounded read.
                    unsafe {
                        ReadFile(pins.client.raw(), Some(&mut bytes), Some(&mut count), None)
                    }
                    .map_err(|e| error("kernel_fixture_read_reply", e))?;
                    if count != 14 || &bytes != b"retained reply" {
                        return Err(EntryTransportError::Malformed);
                    }
                    let message =
                        pins.peer.verify(pins.client.raw(), false, &expected, own, true)?;
                    if message.pid != first.pid || message.birth != first.birth {
                        return Err(EntryTransportError::Authentication);
                    }
                    pins.client.close("kernel_fixture_actual_client_close")?;
                    let mut probe = [0];
                    let mut count = 0;
                    // SAFETY: actual client is checked closed; fixed one-byte EOF output is local to this synchronous call.
                    let eof = unsafe {
                        ReadFile(pins.server.raw(), Some(&mut probe), Some(&mut count), None)
                    }
                    .err()
                    .ok_or(EntryTransportError::InvalidPhase)?;
                    if eof.code() != windows::core::HRESULT::from_win32(ERROR_BROKEN_PIPE.0) {
                        return Err(error("kernel_fixture_eof", eof));
                    }
                    // SAFETY: only after genuine complete read, peer verification and client EOF.
                    unsafe { DisconnectNamedPipe(pins.server.raw()) }
                        .map_err(|e| error("kernel_fixture_final_disconnect", e))?;
                    Ok((message.pid, None))
                })();
            let cleanup = pins.retire();
            let result = match (observed, cleanup) {
                (Ok(value), Ok(())) => value,
                (Err(mut cause), Err(cleanup)) => {
                    cause.retain_cleanup(cleanup);
                    return Err(cause);
                }
                (Err(cause), _) | (_, Err(cause)) => return Err(cause),
            };
            let charge = pins.charge.take().ok_or(EntryTransportError::Capacity)?;
            Ok((result.0, result.1, charge))
        },
    )?;
    let (pid, cause, _charge) = match rig.completion(&mut job)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(Box::new(cause)),
        _ => return Err("Actual fixture terminal differs".into()),
    };
    assert_eq!(pid, std::process::id());
    assert_eq!(
        cause,
        disconnect.then(|| windows::core::HRESULT::from_win32(ERROR_PIPE_NOT_CONNECTED.0))
    );
    Ok(())
}
#[test]
fn actual_disconnect_after_write_discards_unread_reply_and_breaks_kernel_peer_query() -> TestResult
{
    qualify(true)
}
#[test]
fn actual_final_reply_retained_until_client_verified_read_and_checked_close() -> TestResult {
    qualify(false)
}
