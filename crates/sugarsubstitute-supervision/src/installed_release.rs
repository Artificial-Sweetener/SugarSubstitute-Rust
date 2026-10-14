// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select authenticated local release files through the original capture execution owner.
pub(crate) mod allocation;
mod authority;
pub(crate) mod names;
mod outputs;
pub use crate::release_selection::{
    CaptureSelectionRequest, InstalledReleaseOwner, SymbolSelectionRequest,
};
pub use allocation::CaptureCapacityFailure;
pub(crate) use authority::{ReleaseAuthority, SelectedRetirement};
pub use outputs::CaptureModuleOutput;

mod causes;
pub(crate) use allocation::{AllocationOrigin, SharedCaptureLoan};
pub(crate) use causes::{CaptureBoundedCause, CaptureCauseDestination};
pub(crate) use names::CaptureNativeName;
pub(crate) use outputs::{CaptureOutputGuard, CaptureOutputQuota};

pub(crate) use allocation::CaptureScratch;
pub(crate) use names::CaptureTransientName;

mod anchor;
mod bundle;
#[allow(unsafe_code)]
mod crypto;
#[allow(unsafe_code)]
mod current_image;
mod image;
#[cfg(feature = "local-k1-symbol-trust")]
mod local_k1_anchor;
#[cfg(feature = "local-k1-symbol-trust")]
mod local_k1_build;
mod manifest;
mod native_read;
pub use bundle::{
    CaptureBundleLease, CaptureBundleOwner, CaptureBundleParts, CaptureBundleRetention,
    CaptureConstructionFailure, CaptureOwnerRetirementFailure, CaptureRegistrationFacts,
    CaptureRetirementFailure,
};

pub(crate) use names::{CaptureNativeSid, CaptureSidDestination};

pub use bundle::{
    SymbolBundleLease, SymbolBundleOwner, SymbolConstructionFailure, SymbolOwnerRetirementFailure,
    SymbolRegistrationFacts,
};
