// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve original external buffer loans on every pre-dispatch rejection.
use super::*;
use sugarsubstitute_contracts::incident::DiagnosticKey;
pub(super) fn receive<G>(
    peer: DiagnosticKey,
    expected: DiagnosticKey,
    guard: G,
    bytes: impl Fn(&G) -> &[u8],
) -> Result<G, (DiagnosticWireError, G)> {
    let validation = if peer != expected {
        Err(DiagnosticWireError::Identity)
    } else {
        decode(bytes(&guard)).and_then(|frame| {
            if frame.key == expected { Ok(()) } else { Err(DiagnosticWireError::Identity) }
        })
    };
    match validation {
        Ok(()) => Ok(guard),
        Err(error) => Err((error, guard)),
    }
}
pub(super) fn prepare<G>(
    mut guard: G,
    frame: DiagnosticFrame<'_>,
    bytes: impl Fn(&mut G) -> &mut [u8],
) -> Result<(G, usize), (DiagnosticWireError, G)> {
    match encode(frame, bytes(&mut guard)) {
        Ok(length) => Ok((guard, length)),
        Err(error) => Err((error, guard)),
    }
}
