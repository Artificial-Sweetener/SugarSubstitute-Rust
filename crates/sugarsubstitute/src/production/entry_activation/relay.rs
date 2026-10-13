// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route retained actual parent results before another request can overwrite them.
use super::{
    CoordinatorBackend, Result, RetainedRecord, forwarding::Forwarded, progress::with_parent,
};
use crate::production::entry_runner::EntryResources;

pub(super) fn route(
    records: &mut [Option<RetainedRecord>; 64],
    backend: &mut CoordinatorBackend,
    resources: &mut EntryResources<'_>,
) -> Result<()> {
    for source in 0..records.len() {
        let Some(record) = records[source].as_mut().map(|held| held.record.as_mut()) else {
            continue;
        };
        if record.forwarded.is_none() {
            record.forwarded = record
                .parent
                .as_mut()
                .and_then(|parent| parent.take_forwarded_transition())
                .map(Forwarded::new);
        }
        let Some(mut forwarded) = record.forwarded.take() else { continue };
        let delivered: Result<()> = (|| {
            for destination in records.iter_mut().flatten().map(|held| held.record.as_mut()) {
                if let Some(retired) = &destination.retired {
                    forwarded.retire(retired)?;
                } else if destination.parent.is_some() && !destination.closed {
                    forwarded.deliver(destination.run, |transition| {
                        with_parent(destination, backend, resources, |parent, borrowed| {
                            Ok(parent.backend_transition(transition, borrowed)?)
                        })
                    })?;
                }
            }
            Ok(())
        })();
        if !forwarded.complete() {
            records[source].as_mut().ok_or("Actual forwarding source missing")?.record.forwarded =
                Some(forwarded);
        }
        delivered?;
    }
    Ok(())
}
