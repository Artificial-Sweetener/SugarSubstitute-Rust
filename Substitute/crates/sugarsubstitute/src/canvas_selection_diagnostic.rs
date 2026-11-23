// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One closed record per backing tier each canvas selection first shows, relayed to the
//! application log.
//!
//! Choosing an output either restores a retained document, whose finished backings show without
//! new work, or acquires, decodes and renders it again. Each selection records which route it
//! took, how long its source took to load and when each backing tier first reached the screen,
//! so a slow return to a final names whether earlier work was reused.
use std::time::Duration;

/// How a selection obtained its document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Route {
    /// A retained document became current again; retention is a document composition feature.
    #[cfg_attr(not(feature = "native-composition"), allow(dead_code))]
    Retained,
    /// The source was acquired and loaded.
    Loaded,
}

/// The backing tier a frame showed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Tier {
    Fallback,
    Navigation,
    Detail,
}

/// One selection's progress, timed on the session's navigation clock.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectionWatch {
    selection: u64,
    route: Route,
    started: Duration,
    loaded: Option<Duration>,
    shown: [bool; 3],
}

impl SelectionWatch {
    /// Begin watching selection `selection`, offered at `now`.
    pub(crate) const fn start(selection: u64, now: Duration) -> Self {
        Self { selection, route: Route::Loaded, started: now, loaded: None, shown: [false; 3] }
    }

    /// The selection restores a retained document instead of loading.
    #[cfg_attr(not(feature = "native-composition"), allow(dead_code))]
    pub(crate) fn retained(&mut self) {
        self.route = Route::Retained;
    }

    /// The selection's source finished loading at `now`.
    pub(crate) fn loaded(&mut self, selection: u64, now: Duration) {
        if selection == self.selection {
            self.loaded.get_or_insert(now);
        }
    }

    /// A native acceptance of `tier` for `selection` at `now`; the record of the first one per
    /// tier.
    pub(crate) fn shown(&mut self, selection: u64, tier: Tier, now: Duration) -> Option<Record> {
        let index = tier as usize;
        if selection != self.selection || self.shown[index] {
            return None;
        }
        self.shown[index] = true;
        let micros = |value: Duration| u64::try_from(value.as_micros()).unwrap_or(u64::MAX);
        Some(Record {
            selection,
            route: self.route,
            tier,
            load: micros(self.loaded.map_or(Duration::ZERO, |at| at.saturating_sub(self.started))),
            shown: micros(now.saturating_sub(self.started)),
        })
    }
}

/// One tier's first showing for one selection, in whole microseconds from the selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) selection: u64,
    pub(crate) route: Route,
    pub(crate) tier: Tier,
    pub(crate) load: u64,
    pub(crate) shown: u64,
}

impl Record {
    /// Write this record on the child's diagnostic output.
    pub(crate) fn emit(self) {
        tracing::info!(
            operation = "canvas_selection_shown",
            schema = 1,
            selection = self.selection,
            route = ?self.route,
            tier = ?self.tier,
            load_us = self.load,
            shown_us = self.shown
        );
    }

    /// Accept exactly the schema-one line `emit` writes, within the child-output scratch.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 512 {
            return None;
        }
        let text = std::str::from_utf8(bytes).ok()?.trim_end_matches('\r');
        let (prefix, fields) = text.split_once("operation=\"canvas_selection_shown\" ")?;
        if prefix.contains("operation=")
            || !prefix.ends_with("sugarsubstitute::canvas_selection_diagnostic: ")
        {
            return None;
        }
        let mut fields = fields.split_ascii_whitespace();
        let mut field = |name: &str| fields.next()?.strip_prefix(name)?.strip_prefix('=');
        if field("schema")? != "1" {
            return None;
        }
        let selection = number(field("selection")?)?;
        let route = match field("route")? {
            "Retained" => Route::Retained,
            "Loaded" => Route::Loaded,
            _ => return None,
        };
        let tier = match field("tier")? {
            "Fallback" => Tier::Fallback,
            "Navigation" => Tier::Navigation,
            "Detail" => Tier::Detail,
            _ => return None,
        };
        let load = number(field("load_us")?)?;
        let shown = number(field("shown_us")?)?;
        if fields.next().is_some() {
            return None;
        }
        Some(Self { selection, route, tier, load, shown })
    }

    /// Write the relayed selection timing to the application log under the child's run.
    #[cfg(feature = "windows-native-host")]
    pub(crate) fn relay(self, run: sugarsubstitute_contracts::lifecycle::RunKey) {
        let ms = |value: u64| value as f64 / 1000.0;
        tracing::info!(operation = "desktop_child_canvas_selection_shown", run = ?run,
            selection = self.selection, route = ?self.route, tier = ?self.tier,
            load_ms = ms(self.load), shown_ms = ms(self.shown),
            "A canvas selection first showed a backing tier");
    }
}

/// Refuse signs, padding and overflow at the passive wire boundary.
#[cfg(feature = "windows-native-host")]
fn number(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

#[cfg(all(test, feature = "windows-native-host"))]
#[path = "canvas_selection_diagnostic/tests.rs"]
mod tests;
