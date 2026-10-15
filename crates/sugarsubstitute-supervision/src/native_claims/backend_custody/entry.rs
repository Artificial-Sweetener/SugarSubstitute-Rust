// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share the original64 native witness slots across explicit backend/profile purposes.
use super::{profile_registration::ProfileRegistration, registration::Registration};
use crate::ClaimError;
use sugarsubstitute_contracts::{
    backend_custody::ProfileCustodyKey,
    execution::{JobContext, Moment},
    lifecycle::ResourceKey,
};
use sugarsubstitute_execution::backend_custody::BackendCustodyWake;
pub(super) enum Entry {
    Backend(Box<Registration>),
    Profile(Box<ProfileRegistration>),
}
impl Entry {
    pub(super) fn resource(&self) -> Option<ResourceKey> {
        match self {
            Self::Backend(value) => Some(value.resource()),
            Self::Profile(_) => None,
        }
    }
    pub(super) fn profile(&self) -> Option<ProfileCustodyKey> {
        match self {
            Self::Profile(value) => Some(value.key()),
            Self::Backend(_) => None,
        }
    }
    pub(super) fn stop(&mut self, until: Moment) -> Result<(), ClaimError> {
        match self {
            Self::Backend(value) => value.stop(until),
            Self::Profile(value) => value.stop(until),
        }
    }
    pub(super) fn root_signaled(&self) -> bool {
        match self {
            Self::Backend(value) => value.root_signaled(),
            Self::Profile(value) => value.root_signaled(),
        }
    }
    pub(super) fn cleaning(&self) -> bool {
        match self {
            Self::Backend(value) => value.cleaning(),
            Self::Profile(value) => value.cleaning(),
        }
    }
    pub(super) fn settled(&self) -> bool {
        match self {
            Self::Backend(value) => value.settled(),
            Self::Profile(value) => value.settled(),
        }
    }
    pub(super) fn retire(
        &mut self,
        context: &JobContext,
        wake: &BackendCustodyWake,
    ) -> Result<bool, ClaimError> {
        match self {
            Self::Backend(value) => value.retire(context, wake),
            Self::Profile(value) => value.retire(context, wake),
        }
    }
}
