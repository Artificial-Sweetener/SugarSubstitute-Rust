// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain the single process-lifetime delivery outside the actual child custody allocation.

use super::{metadata::ChildMetadata, process_lifetime::ProcessLifetimeCapture};
use crate::{capture_attach::types::CaptureAttachFailure, scope::lock};
use std::sync::{Arc, Mutex, atomic::AtomicUsize};

type ResultSlot = Option<Result<ProcessLifetimeCapture, CaptureAttachFailure>>;

/// Separate delivery allocation never enters child custody or Core Record.
/// Its capsule may retain custody without creating a storage->custody->storage cycle.
#[derive(Clone)]
pub(super) struct Delivery {
    value: Arc<Mutex<DeliveryState>>,
    _metadata: ChildMetadata,
}

struct DeliveryState {
    published: bool,
    result: ResultSlot,
}

#[repr(C)]
struct ControlLayout {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: Mutex<DeliveryState>,
}

impl Delivery {
    /// Preallocate one inert terminal slot under the same original metadata guard.
    pub(super) fn new(metadata: ChildMetadata) -> Self {
        Self {
            value: Arc::new(Mutex::new(DeliveryState { published: false, result: None })),
            _metadata: metadata,
        }
    }

    /// Reject duplicate delivery with the identical owned capsule/cause preserved.
    pub(super) fn publish(
        &self,
        result: Result<ProcessLifetimeCapture, CaptureAttachFailure>,
    ) -> Result<(), Result<ProcessLifetimeCapture, CaptureAttachFailure>> {
        let mut state = lock(&self.value);
        if state.published {
            return Err(result);
        }
        state.published = true;
        state.result = Some(result);
        Ok(())
    }

    /// Consume actual transfer/uncertainty once, without polling a native/provider port.
    pub(super) fn take(&self) -> ResultSlot {
        lock(&self.value).result.take()
    }

    /// Include the complete actual delivery control in the original16KiB preflight.
    pub(super) const fn storage_bytes() -> usize {
        size_of::<ControlLayout>()
    }
}
