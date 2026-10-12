// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode bounded diagnostic commands and retain replay/transfer correlation.
//! Wire values grant no repository, capture, family, destination or presentation authority.
mod codec;
mod command_codec;
mod commands;
mod error;
mod identity;
mod reader;
mod replay;
#[cfg(any(windows, test))]
mod routing;
#[cfg(test)]
mod tests;
mod transfer;
#[cfg(windows)]
mod transport;
mod types;
mod validation;
mod writer;
pub use codec::{decode, encode};
pub use commands::*;
pub use error::DiagnosticWireError;
pub use replay::*;
pub use transfer::*;
#[cfg(windows)]
pub use transport::*;
pub use types::*;
/// Complete encoded frame ceiling, including header.
pub const MAX_FRAME_BYTES: usize = 65_536;
/// Closed binary header ceiling.
pub const MAX_HEADER_BYTES: usize = 1024;
/// Raw borrowed chunk ceiling; no base64 expansion.
pub const MAX_CHUNK_BYTES: usize = 60 * 1024;
