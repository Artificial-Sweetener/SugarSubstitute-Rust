// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit one exact discarded Summary before settling its retained payload group.
mod activation;
mod completion;
mod decision;
pub(super) mod intent;
mod preparation;
mod recovery;
mod retirement;
mod types;
use super::stage::slots::*;
use super::*;
use intent::*;
use source::{canonical, read_by_id, retained_source, stage_by_id, whole};
use sugarsubstitute_contracts::incident::{
    AttachmentInventory, DiscardedPayload, IncidentKey, IncidentRevision, MAX_ATTACHMENTS,
    PayloadDiscard,
};
pub use types::*;
