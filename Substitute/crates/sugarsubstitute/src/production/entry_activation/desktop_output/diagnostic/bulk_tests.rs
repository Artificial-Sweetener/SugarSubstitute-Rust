// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A desktop child's bulk channel refusal crosses the framed stderr relay into the parent log.
use super::super::{BULK_RECORDS, Diagnostic, Observation};
use super::*;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::{
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
    transcript::OutputStream,
};
use sugarsubstitute_desktop::bulk_channel_diagnostic::{Method, Miss, Origin};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Bytes the application formatter writes, as the child's stderr or the parent's log carries them.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Lines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("Captured lines poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Lines {
    fn bytes(&self) -> Result<Vec<u8>> {
        Ok(self.0.lock().map_err(|_| "Captured lines poisoned")?.clone())
    }
}

/// Run `emit` under the application's own routed formatter and return what it wrote.
fn formatted(emit: impl FnOnce()) -> Result<Vec<u8>> {
    let lines = Lines::default();
    let writer = lines.clone();
    let subscriber = crate::diagnostics::fixture_subscriber(move || writer.clone());
    tracing::subscriber::with_default(subscriber, emit);
    lines.bytes()
}

fn run() -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}

const REFUSAL: Record = Record {
    miss: Miss::NonScriptContext,
    context: 3,
    method: Method::Get,
    origin: Origin::Absent,
};

#[test]
fn a_desktop_child_refusal_reaches_the_parent_log_with_its_closed_fields() -> Result {
    let child = formatted(|| REFUSAL.emit())?;
    assert!(child.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
    let mut decoder = Diagnostic::new();
    let mut seen = Vec::new();
    let split = child.len() / 2;
    decoder.feed(OutputStream::Stderr, 1, false, false, &child[..split], |v| seen.push(v));
    assert!(seen.is_empty(), "a partial line was decoded");
    decoder.feed(OutputStream::Stderr, 2, false, false, &child[split..], |v| seen.push(v));
    assert_eq!(seen, [Observation::Bulk(REFUSAL)]);

    let run = run()?;
    let parent = String::from_utf8(formatted(|| relay(run, REFUSAL))?)?;
    for field in [
        "operation=\"desktop_child_bulk_channel\"",
        "reason=non_script_context",
        "context=3",
        "method=GET",
        "origin=absent",
    ] {
        assert!(parent.contains(field), "{field} missing from {parent}");
    }
    assert!(!parent.contains("[redacted]"), "{parent}");
    Ok(())
}

#[test]
fn every_refusal_survives_the_routed_formatter_and_the_relay_budget_holds() -> Result {
    let records: Vec<Record> = [
        Miss::UrlUnreadable,
        Miss::PrefixMismatch,
        Miss::RouteMismatch,
        Miss::MalformedCapability,
        Miss::ForeignOrigin,
        Miss::DownloadUnknown,
        Miss::DownloadExpired,
        Miss::UploadUnknown,
        Miss::UploadRead,
    ]
    .into_iter()
    .map(|miss| Record { miss, context: -1, method: Method::Post, origin: Origin::Other })
    .collect();
    let child = formatted(|| records.iter().for_each(|record| record.emit()))?;
    let mut decoder = Diagnostic::new();
    let mut seen = Vec::new();
    decoder.feed(OutputStream::Stderr, 1, false, false, &child, |v| seen.push(v));
    let decoded: Vec<Observation> = records.iter().copied().map(Observation::Bulk).collect();
    assert_eq!(seen, decoded);

    let line = formatted(|| REFUSAL.emit())?;
    let mut decoder = Diagnostic::new();
    let mut relayed = 0_usize;
    for sequence in 1..=u64::from(BULK_RECORDS) + 8 {
        decoder.feed(OutputStream::Stderr, sequence, false, false, &line, |v| {
            relayed += usize::from(matches!(v, Observation::Bulk(_)));
        });
    }
    assert_eq!(relayed, usize::from(BULK_RECORDS));
    Ok(())
}
