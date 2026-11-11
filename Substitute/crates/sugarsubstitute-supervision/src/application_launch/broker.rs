// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer a verified launch to its native creator without transferring engine containment.
mod receiver;
mod sender;
pub(super) mod source;
mod wire;
use super::*;
pub(super) use receiver::CapturedPins;
pub use receiver::{LaunchReceiver, LaunchResult};
pub use sender::{ActivationSender, capture_activation};
pub use source::LaunchOffer;

/// Covers bounded framing, decoded handle identities, native duplicates and command storage.
/// Retained transfer custody keeps this reservation until its final owner retires.
pub const TRANSFER_RESERVATION_BYTES: u64 = (3 * wire::MAX_BYTES
    + (wire::MAX_PINS + 2) * (std::mem::size_of::<Handle>() + 3 * std::mem::size_of::<u64>())
    + 1024 * 1024) as u64;

/// Keep every transfer operation inside its original admitted worker lifetime.
fn admit(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(ClaimError::invalid("launch_transfer_admission"));
    }
    Ok(())
}
