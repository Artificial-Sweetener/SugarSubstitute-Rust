// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain only fixed raw causes; every escaped source is paired outside with whole custody.
use super::{EntryPreparationInputError, error::PreparationSource};
use crate::scope::lock;
use std::sync::Mutex;

pub(super) struct PreparationSources(Mutex<Sources>);
struct Sources {
    retained: [Option<PreparationSource>; 9],
    observed: u64,
}
impl PreparationSources {
    pub(super) fn new() -> Self {
        Self(Mutex::new(Sources { retained: std::array::from_fn(|_| None), observed: 0 }))
    }
    pub(super) fn retain(
        &self,
        cause: EntryPreparationInputError,
    ) -> (Option<PreparationSource>, Option<EntryPreparationInputError>) {
        let mut pending = Some(cause);
        let retained = {
            let mut sources = lock(&self.0);
            sources.observed = sources.observed.saturating_add(1);
            if let Some(slot) = sources.retained.iter_mut().find(|slot| slot.is_none()) {
                if let Some(cause) = pending.take() {
                    let source = PreparationSource::shared(cause);
                    *slot = Some(source.clone());
                    Some(source)
                } else {
                    None
                }
            } else {
                sources.retained.iter().find_map(|source| source.as_ref().cloned())
            }
        };
        // The caller owns the outside whole guard before destroying an omitted cause.
        (retained, pending)
    }
    pub(super) fn omitted(&self) -> u64 {
        lock(&self.0).observed.saturating_sub(9)
    }
}
