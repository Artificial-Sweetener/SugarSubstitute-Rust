// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the single unchanged SSCE1 Launch body grammar for frames and startup views.
mod decode;
mod encode;
use super::{
    EntryLaunchRequest, EntryLaunchRequestView, EntryTransportError, HEADER_BYTES,
    MAX_ENTRY_FRAME_BYTES, Reader, Writer, raw_windows,
};
pub(crate) use decode::decode;
pub(crate) use encode::{encode, encoded_length};
const MAX_BODY_BYTES: usize = MAX_ENTRY_FRAME_BYTES - HEADER_BYTES;
