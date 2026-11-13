// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize native matching data; these fixtures create no file, token or permission.
use super::*;
fn facts() -> (Query, abi::Facts) {
    let subject = abi::Subject {
        tag: 1,
        namespace: [1; 32],
        epoch: [2; 16],
        reserved: 0,
        instance: 3,
        generation: 4,
    };
    let mut build = [0; 64];
    build[..4].copy_from_slice(b"k1_1");
    let query = Query {
        subject,
        build_length: 4,
        build,
        uuid: Some([5; 16]),
        directories: [[6; 24], [7; 24]],
    };
    let value = abi::Facts {
        subject,
        uuid: [5; 16],
        build_length: 4,
        build,
        reserved: 0,
        source_bytes: 64,
        source_identity: [8; 24],
        temporary_bytes: 0,
        entries: 2,
        complete: 1,
    };
    (query, value)
}
fn image() -> (Query, abi::Facts, abi::Operation, abi::Preparation) {
    let (query, source) = facts();
    let operation = abi::Operation { namespace: [1; 32], epoch: [2; 16], id: 10 };
    let mut value = abi::Preparation::empty();
    value.version = 1;
    value.historical = operation;
    value.subject = source.subject;
    value.uuid = source.uuid;
    value.build_length = source.build_length;
    value.build = source.build;
    value.database_identity = query.directories[0];
    value.metrics_identity = query.directories[1];
    value.source = abi::FileFacts { identity: source.source_identity, bytes: 64, sha256: [9; 32] };
    value.metadata = abi::FileFacts { identity: [10; 24], bytes: 114, sha256: [11; 32] };
    value.original_stage = abi::StageFacts {
        role: 1,
        reserved: 0,
        file: abi::FileFacts { identity: [12; 24], bytes: 114, sha256: [11; 32] },
    };
    value.candidate_stage = abi::StageFacts {
        role: 2,
        reserved: 0,
        file: abi::FileFacts { identity: [13; 24], bytes: 58, sha256: [14; 32] },
    };
    (query, source, operation, value)
}
#[test]
fn query_rejects_changed_full_subject_uuid_build_and_source_bounds() {
    let (query, value) = facts();
    assert!(query.matches(&value));
    let changes: [fn(&mut abi::Facts); 7] = [
        |v| v.subject.epoch[0] ^= 1,
        |v| v.subject.generation += 1,
        |v| v.uuid[0] ^= 1,
        |v| v.build[0] ^= 1,
        |v| v.source_bytes = 256 * 1024 * 1024 + 1,
        |v| v.source_identity = [0; 24],
        |v| v.complete = 2,
    ];
    for change in changes {
        let mut candidate = value;
        change(&mut candidate);
        assert!(!query.matches(&candidate));
    }
}
#[test]
fn preparation_requires_exact_original_and_distinct_checked_candidate_before_journal() {
    let (query, source, operation, value) = image();
    assert!(preparation(&value, operation, &query, &source));
    let changes: [fn(&mut abi::Preparation); 10] = [
        |v| v.historical.id += 1,
        |v| v.database_identity[0] ^= 1,
        |v| v.metrics_identity[0] ^= 1,
        |v| v.source.identity[0] ^= 1,
        |v| v.source.bytes += 1,
        |v| v.original_stage.file.sha256[0] ^= 1,
        |v| v.candidate_stage.file.identity = v.original_stage.file.identity,
        |v| v.candidate_stage.file.identity = v.metadata.identity,
        |v| v.candidate_stage.file.bytes += 1,
        |v| v.candidate_stage.role = 1,
    ];
    for change in changes {
        let mut candidate = value;
        change(&mut candidate);
        assert!(!preparation(&candidate, operation, &query, &source));
    }
}

#[test]
fn preparation_rejects_nonproducer_record_width_and_short_metadata_header() {
    let (query, source, operation, value) = image();
    assert!(preparation(&value, operation, &query, &source));
    for removed in [55_u64, 64] {
        let mut changed = value;
        changed.candidate_stage.file.bytes = changed.metadata.bytes - removed;
        assert!(!preparation(&changed, operation, &query, &source));
    }
    let mut short = value;
    short.metadata.bytes = 71;
    short.original_stage.file.bytes = 71;
    short.candidate_stage.file.bytes = 15;
    assert!(!preparation(&short, operation, &query, &source));
}
