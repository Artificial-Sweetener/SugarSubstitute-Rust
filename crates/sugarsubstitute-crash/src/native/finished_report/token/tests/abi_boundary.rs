// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replace only the external CPP producer with isolated fixed scripted result cells.
use super::super::abi;
use std::sync::Mutex;
#[derive(Clone, Copy)]
struct Script {
    key: usize,
    mode: u8,
    reads: u32,
    retires: u32,
    facts: abi::Facts,
    directories: [[u8; 24]; 2],
    dispositions: u32,
}
static SCRIPTS: Mutex<[Option<Script>; 32]> = Mutex::new([None; 32]);
pub(super) fn counts(key: usize) -> (u32, u32) {
    SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .flatten()
        .find(|s| s.key == key)
        .map_or((0, 0), |s| (s.reads, s.retires))
}
unsafe extern "C" fn open(input: *const abi::Open<'_>, output: *mut abi::Output) {
    // SAFETY: Token supplies these exclusively borrowed live ABI cells synchronously.
    let (input, output) = unsafe { (&*input, &mut *output) };
    let mode = match &input.build[..input.build_length as usize] {
        b"foreign" => 1,
        b"noowner" => 2,
        b"invalid" => 3,
        b"uncertain" => 5,
        b"copy" | b"partialcopy" | b"copy-live" | b"copy-handle" | b"copy-failedretire" => 6,
        _ => 4,
    };
    let key = input.database;
    let mut scripts = SCRIPTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let slot = scripts.iter_mut().find(|s| s.is_none_or(|s| s.key == key));
    let retained = slot;
    *output = abi::Output::empty();
    output.version = 1;
    if mode == 2 || mode == 5 {
        output.status = 7;
        output.primary_error = 8;
        output.primary_origin = 1;
        output.retired = u32::from(mode == 2);
        return;
    }
    if mode == 3 {
        output.retired = 1;
        return;
    }
    output.lease = key;
    output.handles = 1;
    output.facts_valid = 1;
    output.facts = abi::Facts {
        subject: input.subject,
        uuid: [5; 16],
        build_length: input.build_length,
        build: input.build,
        reserved: 0,
        source_bytes: 64,
        source_identity: [31; 24],
        temporary_bytes: 0,
        entries: 1,
        complete: 1,
    };
    if let Some(slot) = retained {
        *slot = Some(Script {
            key,
            mode,
            reads: 0,
            retires: 0,
            facts: output.facts,
            directories: [input.database_identity, input.metrics_identity],
            dispositions: 0,
        });
    }
}
unsafe extern "C" fn read(input: *const abi::Read<'_>, output: *mut abi::Output) {
    // SAFETY: Same retained input and exclusive output passed by the real Token call.
    let (input, output) = unsafe { (&*input, &mut *output) };
    let mut scripts = SCRIPTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let script = scripts.iter_mut().flatten().find(|s| s.key == input.call.lease);
    *output = abi::Output::empty();
    output.version = 1;
    if let Some(script) = script {
        script.reads += 1;
        output.lease = script.key;
        if script.mode == 6 {
            // SAFETY: The genuine Token borrows exactly this output capacity for the call.
            let bytes =
                unsafe { std::slice::from_raw_parts_mut(input.output, input.capacity as usize) };
            bytes.fill(0x5a);
            output.read_bytes = bytes.len() as u64;
        } else if script.mode == 1 {
            output.lease = script.key + script.reads as usize;
        } else {
            output.lease = 0;
            output.retired = 1;
            output.read_bytes = input.capacity as u64 + 1;
        }
    }
}
unsafe extern "C" fn retire(input: *const abi::Call<'_>, output: *mut abi::Output) {
    // SAFETY: Real Token supplies the known input and exclusively borrowed output.
    let (input, output) = unsafe { (&*input, &mut *output) };
    if let Some(script) = SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter_mut()
        .flatten()
        .find(|s| s.key == input.lease)
    {
        script.retires += 1;
    }
    *output = abi::Output::empty();
    output.version = 1;
    output.retired = 1;
}
unsafe extern "C" fn prepare(input: *const abi::Prepare<'_>, output: *mut abi::PrepareOutput) {
    use sha2::{Digest, Sha256};
    // SAFETY: Actual Token owns these live input/output cells for this synchronous call.
    let (input, output) = unsafe { (&*input, &mut *output) };
    let scripts = SCRIPTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(script) = scripts.iter().flatten().find(|s| s.key == input.call.lease) else { return };
    output.state = abi::Output::empty();
    output.state.version = 1;
    output.state.lease = script.key;
    output.state.handles = 1;
    let mut prepared = abi::Preparation::empty();
    prepared.version = 1;
    prepared.historical = input.operation;
    prepared.subject = script.facts.subject;
    prepared.uuid = script.facts.uuid;
    prepared.build_length = script.facts.build_length;
    prepared.build = script.facts.build;
    prepared.database_identity = script.directories[0];
    prepared.metrics_identity = script.directories[1];
    prepared.source = abi::FileFacts {
        identity: script.facts.source_identity,
        bytes: 64,
        sha256: Sha256::digest([0x5a; 64]).into(),
    };
    prepared.metadata = abi::FileFacts { identity: [41; 24], bytes: 114, sha256: [42; 32] };
    prepared.original_stage = abi::StageFacts {
        role: 1,
        reserved: 0,
        file: abi::FileFacts { identity: [43; 24], ..prepared.metadata },
    };
    prepared.candidate_stage = abi::StageFacts {
        role: 2,
        reserved: 0,
        file: abi::FileFacts { identity: [44; 24], bytes: 58, sha256: [45; 32] },
    };
    output.valid = 1;
    output.reserved = 0;
    output.preparation = prepared;
}
unsafe extern "C" fn discard(input: *const abi::Discard<'_>, output: *mut abi::DispositionOutput) {
    // SAFETY: The genuine Token exclusively supplies the pinned input/permit/result cells.
    let (input, output) = unsafe { (&*input, &mut *output) };
    // SAFETY: The same exclusive pinned Token input retains its permit for this callback.
    let permit = unsafe { &*input.permit };
    assert_eq!(permit.authorization, 1);
    assert_eq!(permit.plan_sha256, permit.intent.sha256);
    assert_eq!(permit.payload.sha256, permit.prepared.source.sha256);
    let mut scripts = SCRIPTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut script = scripts.iter_mut().flatten().find(|s| s.key == input.call.lease);
    if let Some(script) = &mut script {
        script.dispositions += 1;
    }
    output.state = abi::Output::empty();
    output.state.version = 1;
    output.state.retired = 1;
    output.state.reclaimed_bytes = permit.prepared.source.bytes;
    if let Some(script) = script {
        output.state.facts_valid = 1;
        output.state.facts = script.facts;
    }
    output.source = 2;
    output.metadata = 2;
    output.stages_absent = 3;
    output.reserved = 0;
    match &permit.prepared.build[..permit.prepared.build_length as usize] {
        b"copy-live" => {
            output.state.retired = 0;
            output.state.lease = input.call.lease;
        }
        b"copy-handle" => output.state.handles = 1,
        b"copy-failedretire" => {
            output.state.status = 7;
            output.state.primary_error = 8;
            output.state.primary_origin = 1;
            output.state.facts_valid = 0;
            output.state.reclaimed_bytes = 0;
            output.source = 4;
            output.metadata = 3;
            output.stages_absent = 0;
        }
        _ => {}
    }
}
pub(super) fn dispositions(key: usize) -> u32 {
    SCRIPTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .flatten()
        .find(|s| s.key == key)
        .map_or(0, |s| s.dispositions)
}
unsafe extern "C" fn recover(_: *const abi::Recover<'_>, _: *mut abi::DispositionOutput) {}
pub(super) fn api() -> abi::Api {
    abi::Api { open, read, retire, prepare, discard, recover }
}
