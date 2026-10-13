// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded returned outcomes before lazily observing post-step control.
use std::io::Read;
use sugarsubstitute_comfy::configuration::MAX_STARTUP_CONFIGURATION_BYTES;

pub(super) enum StepFailure<E, C> {
    Primary { source: E, after: Option<C> },
    Control(C),
}
pub(super) fn completed_step<T, E, C>(
    outcome: Result<T, E>,
    after: impl FnOnce() -> Result<(), C>,
) -> Result<T, StepFailure<E, C>> {
    // The returned operation is retained on this frame throughout post observation.
    let post = after();
    match (outcome, post) {
        (Err(source), after) => Err(StepFailure::Primary { source, after: after.err() }),
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(cause)) => Err(StepFailure::Control(cause)),
    }
}
pub(super) enum ReadFailure<C> {
    Control(C),
    Read { source: std::io::Error, after: Option<C> },
    Capacity(std::collections::TryReserveError),
    TooLarge,
}
pub(super) fn read_bounded<C>(
    mut source: impl Read,
    mut before: impl FnMut() -> Result<(), C>,
    mut after: impl FnMut() -> Result<(), C>,
) -> Result<Vec<u8>, ReadFailure<C>> {
    before().map_err(ReadFailure::Control)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(MAX_STARTUP_CONFIGURATION_BYTES + 1).map_err(ReadFailure::Capacity)?;
    if bytes.capacity() != MAX_STARTUP_CONFIGURATION_BYTES + 1 {
        return Err(ReadFailure::TooLarge);
    }
    bytes.resize(MAX_STARTUP_CONFIGURATION_BYTES + 1, 0);
    let mut used = 0;
    loop {
        before().map_err(ReadFailure::Control)?;
        let returned = source.read(&mut bytes[used..]);
        let actual = completed_step(returned, &mut after).map_err(|failure| match failure {
            StepFailure::Primary { source, after } => ReadFailure::Read { source, after },
            StepFailure::Control(cause) => ReadFailure::Control(cause),
        })?;
        if actual == 0 {
            bytes.truncate(used);
            return Ok(bytes);
        }
        used += actual;
        if used > MAX_STARTUP_CONFIGURATION_BYTES {
            return Err(ReadFailure::TooLarge);
        }
    }
}
