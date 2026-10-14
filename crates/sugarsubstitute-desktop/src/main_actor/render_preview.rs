// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep bounded native preview storage and callback custody with the actual main owner.

mod buffer;
#[cfg(feature = "k1-native-qualification")]
mod private_desktop;
mod stream;

#[cfg(feature = "k1-native-qualification")]
mod actor;
#[cfg(feature = "k1-native-qualification")]
mod capture;
#[cfg(feature = "k1-native-qualification")]
pub(super) use capture::Capture;
