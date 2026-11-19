// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide which of the host's own lines the page is owed now: the close save, a ready or
//! failed update, and the start-up update messages. Each is said once.

use super::session_notice_page::HostLine;
use sugarsubstitute_contracts::{
    host_session_notice::NoticeTone,
    localization::{Messages, updates::UpdateMessages},
    update::status::{StartupNotice, UpdateStatus},
};

/// What the host has already told the page, so that no line is repeated.
#[derive(Debug, Default)]
pub(crate) struct LineSources {
    /// The close save under way has been announced.
    saving: bool,
    /// The version whose readiness has been announced.
    ready: Option<String>,
    /// The failure now reported has been announced.
    failed: bool,
}

impl LineSources {
    /// The lines owed for this turn's facts, in `words`' language.
    ///
    /// `saving` is whether the close save is running; `status` is the coordinator's newest
    /// update status and `failed` whether it has a report worth reading. A ready update is
    /// announced once per version, a failure once until it clears, and the close save once.
    pub(crate) fn due(
        &mut self,
        words: &'static Messages,
        saving: bool,
        status: Option<&UpdateStatus>,
        failed: bool,
    ) -> Vec<HostLine> {
        let mut lines = Vec::new();
        if saving && !self.saving {
            lines.push(line(NoticeTone::Info, words.closing.saving, true));
        }
        self.saving = saving;
        let ready = status.and_then(UpdateStatus::ready);
        if let Some(version) = ready
            && self.ready.as_deref() != Some(version)
        {
            self.ready = Some(version.to_owned());
            lines.push(line(NoticeTone::Info, words.updates.ready_line, true));
        }
        // A ready update outranks a later failure, because restarting still applies it.
        let failed = failed && ready.is_none();
        if failed && !self.failed {
            lines.push(line(NoticeTone::Warn, words.updates.failed_line, true));
        }
        self.failed = failed;
        lines
    }
}

/// The line for one start-up update message, in `words`' language.
pub(crate) fn startup_line(words: &'static Messages, notice: &StartupNotice) -> HostLine {
    let updates = words.updates;
    match notice {
        StartupNotice::Updated(version) => HostLine {
            tone: NoticeTone::Info,
            text: UpdateMessages::with_version(updates.updated, version),
            lasting: false,
        },
        StartupNotice::PreviousVersionRunning => {
            line(NoticeTone::Warn, updates.previous_running, true)
        }
        StartupNotice::Repairing => line(NoticeTone::Warn, updates.repairing, true),
        StartupNotice::StorageSetAside => line(NoticeTone::Warn, updates.storage_set_aside, true),
        StartupNotice::Sugarcubes(report) => {
            let attention = report.needs_attention();
            HostLine {
                tone: if attention { NoticeTone::Warn } else { NoticeTone::Info },
                text: report
                    .notice(words.pack_updates)
                    .unwrap_or_else(|| words.pack_updates.report_title.to_owned()),
                lasting: attention,
            }
        }
    }
}

fn line(tone: NoticeTone, text: &str, lasting: bool) -> HostLine {
    HostLine { tone, text: text.to_owned(), lasting }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::localization::messages;

    #[test]
    fn the_close_save_is_announced_once_per_close() {
        let words = messages("en");
        let mut sources = LineSources::default();
        assert!(sources.due(words, false, None, false).is_empty());
        let said = sources.due(words, true, None, false);
        assert_eq!(said, vec![line(NoticeTone::Info, words.closing.saving, true)]);
        assert!(sources.due(words, true, None, false).is_empty(), "said once while it runs");
    }

    #[test]
    fn a_failure_is_announced_once_until_it_clears() {
        let words = messages("en");
        let mut sources = LineSources::default();
        let said = sources.due(words, false, None, true);
        assert_eq!(said, vec![line(NoticeTone::Warn, words.updates.failed_line, true)]);
        assert!(sources.due(words, false, None, true).is_empty());
        assert!(sources.due(words, false, None, false).is_empty());
        assert_eq!(sources.due(words, false, None, true).len(), 1, "a new failure is said again");
    }

    #[test]
    fn start_up_messages_read_as_the_user_expects() {
        let words = messages("en");
        let updated = startup_line(words, &StartupNotice::Updated("2.4.1".into()));
        assert!(updated.text.contains("2.4.1"));
        assert_eq!((updated.tone, updated.lasting), (NoticeTone::Info, false));
        let previous = startup_line(words, &StartupNotice::PreviousVersionRunning);
        assert_eq!(previous, line(NoticeTone::Warn, words.updates.previous_running, true));
        for notice in [StartupNotice::Repairing, StartupNotice::StorageSetAside] {
            let said = startup_line(words, &notice);
            assert_eq!((said.tone, said.lasting), (NoticeTone::Warn, true));
            assert!(!said.text.is_empty());
        }
    }
}
