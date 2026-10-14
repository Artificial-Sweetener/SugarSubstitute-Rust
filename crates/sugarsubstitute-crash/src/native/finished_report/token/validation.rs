// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check returned matching data against the same query and actual prepared source.
use super::abi;
pub(super) struct Query {
    subject: abi::Subject,
    build_length: u32,
    build: [u8; 64],
    uuid: Option<[u8; 16]>,
    pub(super) directories: [[u8; 24]; 2],
}
impl Query {
    pub(super) fn from_plan(plan: &crate::incident::CaptureDispositionPlan) -> Self {
        let mut build = [0; 64];
        build[..plan.build().as_str().len()].copy_from_slice(plan.build().as_str().as_bytes());
        Self {
            subject: super::calls::subject(plan.incident()),
            build_length: plan.build().as_str().len() as u32,
            build,
            uuid: Some(*plan.uuid().bytes()),
            directories: [plan.database_identity(), plan.metrics_identity()],
        }
    }
    pub(super) fn new(input: &abi::Open<'_>) -> Self {
        Self {
            subject: input.subject,
            build_length: input.build_length,
            build: input.build,
            uuid: (input.has_uuid == 1).then_some(input.optional_uuid),
            directories: [input.database_identity, input.metrics_identity],
        }
    }
    pub(super) fn matches(&self, facts: &abi::Facts) -> bool {
        facts.subject == self.subject
            && facts.build_length == self.build_length
            && facts.build == self.build
            && facts.uuid != [0; 16]
            && self.uuid.is_none_or(|uuid| uuid == facts.uuid)
            && facts.reserved == 0
            && facts.source_identity != [0; 24]
            && facts.source_bytes > 0
            && facts.source_bytes <= 256 * 1024 * 1024
            && facts.entries <= 4096
            && facts.complete <= 1
    }
}
fn file(file: &abi::FileFacts, maximum: u64) -> bool {
    file.identity != [0; 24] && file.sha256 != [0; 32] && file.bytes > 0 && file.bytes <= maximum
}
pub(super) fn preparation(
    value: &abi::Preparation,
    operation: abi::Operation,
    query: &Query,
    source: &abi::Facts,
) -> bool {
    let original = &value.original_stage;
    let candidate = &value.candidate_stage;
    value.version == 1
        && value.reserved == 0
        && value.reserved_build == 0
        && value.historical == operation
        && value.subject == source.subject
        && value.uuid == source.uuid
        && value.build_length == source.build_length
        && value.build == source.build
        && value.database_identity == query.directories[0]
        && value.metrics_identity == query.directories[1]
        && file(&value.source, 256 * 1024 * 1024)
        && value.source.identity == source.source_identity
        && value.source.bytes == source.source_bytes
        && file(&value.metadata, 1024 * 1024)
        && value.metadata.bytes >= 80
        && file(&original.file, 1024 * 1024)
        && file(&candidate.file, 1024 * 1024)
        && original.role == 1
        && candidate.role == 2
        && original.reserved == 0
        && candidate.reserved == 0
        && original.file.bytes == value.metadata.bytes
        && original.file.sha256 == value.metadata.sha256
        && candidate.file.bytes == value.metadata.bytes - 64
        && original.file.sha256 != candidate.file.sha256
        && original.file.identity != candidate.file.identity
        && [original.file.identity, candidate.file.identity]
            .iter()
            .all(|id| *id != value.metadata.identity && *id != value.source.identity)
}
#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;
