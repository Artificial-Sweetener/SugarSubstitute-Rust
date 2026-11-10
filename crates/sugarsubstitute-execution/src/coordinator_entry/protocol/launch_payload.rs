// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the single Launch body grammar for frames and startup views.
mod decode;
mod encode;
mod profile;
use super::{
    EntryLaunchRequest, EntryLaunchRequestView, EntryTransportError, HEADER_BYTES,
    MAX_ENTRY_FRAME_BYTES, Reader, Writer, raw_windows,
};
pub(crate) use decode::decode;
pub(crate) use encode::{encode, encoded_length};
pub(crate) use profile::{profile_length, project_profile};
const MAX_BODY_BYTES: usize = MAX_ENTRY_FRAME_BYTES - HEADER_BYTES;
