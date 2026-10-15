// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test exact public codec behavior without any native peer or process effects.
use super::*;
use std::{error::Error, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{LaunchConfiguration, LoopbackOrigin},
    execution::Generation,
    lifecycle::*,
    startup::{LaunchId, MainIdentity},
};
type TestResult = Result<(), Box<dyn Error>>;
fn header(kind: u16) -> Result<WireHeader, Box<dyn Error>> {
    Ok(WireHeader {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([0x41; 32]),
                epoch: CoordinatorEpoch::new([0x42; 16])?,
            },
            instance: InstanceId::new(7)?,
            generation: RunGeneration::new(9)?,
        },
        connection: if kind == 1 { None } else { Some(ConnectionId::new(11)?) },
        request: if matches!(kind, 1 | 2 | 9) { None } else { Some(RequestId::new(13)?) },
        deadline_qpc: 9_007_199_254_740_993,
    })
}
fn encoded(payload: WirePayload<'_>) -> Result<Vec<u8>, Box<dyn Error>> {
    let message = WireMessage { header: header(payload.kind())?, payload };
    let mut output = vec![0xCC; MAX_FRAME_BYTES];
    let written = FrameCodec::encode(&message, &mut output)?;
    assert!(output[written..].iter().all(|byte| *byte == 0xCC));
    output.truncate(written);
    Ok(output)
}
fn lease() -> Result<LeaseReference, Box<dyn Error>> {
    Ok(LeaseReference {
        resource: ResourceId::new(17)?,
        generation: ResourceGeneration::new(19)?,
        lease: LeaseId::new(23)?,
    })
}
fn managed(arguments: Vec<String>) -> Result<BackendTarget, Box<dyn Error>> {
    Ok(BackendTarget::Managed(LaunchConfiguration::new(
        PathBuf::from("E:\\甜\\e\u{301}😀 root"),
        PathBuf::from("E:\\Py\\python.exe"),
        arguments,
        8188,
    )?))
}
fn set_length(frame: &mut [u8]) {
    let length = frame.len() as u32;
    frame[8..12].copy_from_slice(&length.to_le_bytes());
}
mod framing;
mod payloads;
mod targets;

fn main() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: header(4)?.run,
        launch: LaunchId::new(37)?,
        navigation: Generation::new(41)?,
    })
}
mod extensions;

mod output;

mod admission;
mod backend_recovery;
