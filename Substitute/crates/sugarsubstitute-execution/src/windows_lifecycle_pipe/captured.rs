// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own captured native startup source admission and immutable section matching data.
pub(crate) mod backend;
#[allow(unsafe_code)]
pub(crate) mod child;
pub(crate) mod codec;
pub(crate) mod parent;
pub(crate) use crate::lifecycle_pipe::captured_admission::AdmissionBinding;
#[cfg(test)]
mod codec_tests;

#[cfg(test)]
mod backend_codec_tests;

#[cfg(test)]
mod mapping_install_tests;

#[cfg(test)]
mod recovery_codec_tests;
