// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream a bounded PNG from the exact connected Comfy peer into retained cache custody.

use super::*;
use std::io::{self, Write};
use sugarsubstitute_supervision::{
    backend_listener::ConnectedLoopback,
    cache::{Namespace, storage_key},
};

pub(super) fn fetch(
    state: &mut State,
    transfer: &ComfyOutputTransfer,
    cache: RuntimeCache,
    now: u64,
    context: &JobContext,
) -> Result<CacheReader<'static>, OutputTransferError> {
    state.claim = Some(
        context
            .memory()
            .claim(RETAINED_BYTES)
            .map_err(|cause| OutputTransferError::new("output_admission", cause))?,
    );
    state.workspace = Some(
        TcpOwnerWorkspace::for_output(context)
            .map_err(|cause| OutputTransferError::new("output_tcp_workspace", cause))?,
    );
    let port = transfer.origin.port().get();
    state
        .http
        .connect_loopback(port, context)
        .map_err(|cause| OutputTransferError::new("output_connection", cause))?;
    state
        .http
        .request_loopback(transfer.target.as_str(), context)
        .map_err(|cause| OutputTransferError::new("output_request", cause))?;
    state
        .http
        .send_loopback(context)
        .map_err(|cause| OutputTransferError::new("output_send", cause))?;
    state
        .http
        .receive(context)
        .map_err(|cause| OutputTransferError::new("output_receive", cause))?;
    let before = connection(state, port, context)?;
    qualify(state, before, transfer.peer, context)?;
    let response = state
        .http
        .response(context)
        .map_err(|cause| OutputTransferError::new("output_response", cause))?;
    if response.status != 200 {
        return Err(OutputTransferError::new(
            "output_status",
            io::Error::other(format!("Comfy returned HTTP {}", response.status)),
        ));
    }
    let length =
        crate::readiness_transport::headers::parse_png_length(&response.headers, transfer.maximum)
            .map_err(|cause| OutputTransferError::new("output_headers", cause))?;
    let identity = format!(
        "comfy-output-v1:{}:{}:{}:{}",
        port,
        transfer.peer.process_id,
        transfer.peer.creation_time_100ns,
        transfer.target.as_str()
    );
    let key = storage_key(identity.as_bytes());
    let mut entry = cache
        .fresh(Namespace::ComfyOutputs, &key, length, now, context)
        .map_err(|cause| OutputTransferError::new("output_cache_admission", cause))?;
    entry
        .write_with("output.png", length, context, |output| {
            copy_png(length, output, |buffer| {
                state.http.read(buffer, context).map_err(io::Error::other)
            })
        })
        .map_err(|cause| OutputTransferError::new("output_body", cause))?;
    let after = connection(state, port, context)?;
    if after != before {
        return Err(OutputTransferError::invalid("output_connection_changed"));
    }
    qualify(state, after, transfer.peer, context)?;
    entry.publish(context).map_err(|cause| OutputTransferError::new("output_publish", cause))?;
    let reader = entry
        .into_file_reader("output.png", context)
        .map_err(|cause| OutputTransferError::new("output_pin", cause))?;
    tracing::info!(
        operation = "comfy_output_acquired",
        peer_process_id = transfer.peer.process_id.get(),
        encoded_bytes = length,
        "Selected Comfy output acquired from the verified peer"
    );
    Ok(reader)
}

pub(super) fn copy_png(
    length: u64,
    output: &mut dyn Write,
    mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
) -> io::Result<()> {
    let mut scratch = [0u8; 32 * 1024];
    let mut signature = [0u8; 8];
    let mut total = 0u64;
    loop {
        let count = read(&mut scratch)?;
        if count == 0 {
            break;
        }
        if count > scratch.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Comfy output reader returned an invalid byte count",
            ));
        }
        let next =
            total.checked_add(count as u64).filter(|value| *value <= length).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Comfy output exceeded its declared length",
                )
            })?;
        if total < signature.len() as u64 {
            let start = total as usize;
            let copied = count.min(signature.len() - start);
            signature[start..start + copied].copy_from_slice(&scratch[..copied]);
        }
        output.write_all(&scratch[..count])?;
        total = next;
    }
    if total != length || signature != *b"\x89PNG\r\n\x1a\n" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Comfy output is truncated or not PNG",
        ));
    }
    Ok(())
}

fn connection(
    state: &mut State,
    port: u16,
    context: &JobContext,
) -> Result<ConnectedLoopback, OutputTransferError> {
    let (client, server) = state
        .http
        .loopback_tuple(port, context)
        .map_err(|cause| OutputTransferError::new("output_connected_tuple", cause))?;
    ConnectedLoopback::new(client, server)
        .map_err(|cause| OutputTransferError::new("output_connected_tuple", cause))
}

fn qualify(
    state: &mut State,
    connection: ConnectedLoopback,
    expected: BackendPeer,
    context: &JobContext,
) -> Result<(), OutputTransferError> {
    let workspace = state
        .workspace
        .as_mut()
        .ok_or_else(|| OutputTransferError::invalid("output_tcp_workspace"))?;
    let actual = state
        .listener
        .qualify_connection(connection, workspace, context)
        .map_err(|cause| OutputTransferError::new("output_peer", cause))?;
    if !matches_peer(actual.process_id(), actual.creation_time_100ns(), expected) {
        return Err(OutputTransferError::invalid("output_peer_changed"));
    }
    Ok(())
}

pub(super) fn matches_peer(pid: u32, birth: u64, expected: BackendPeer) -> bool {
    pid == expected.process_id.get() && birth == expected.creation_time_100ns.get()
}
