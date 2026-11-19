// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what one main window's page is told about the host's update report, and which of
//! its requests to show it still stands. This owner holds no native resources.

/// Whether the application has a report, the revision that names that state to the page, and
/// a request to show it that the application has not yet taken.
#[derive(Debug, Default)]
pub(crate) struct ReportOffer {
    revision: u64,
    available: bool,
    asked: bool,
}

impl ReportOffer {
    /// Adopt this turn's fact; true when the page must be told. A report that went away takes
    /// any request for it along.
    pub(crate) fn observe(&mut self, available: bool) -> bool {
        if available == self.available {
            return false;
        }
        self.available = available;
        self.revision += 1;
        self.asked &= available;
        true
    }

    /// The state the page is told.
    pub(crate) const fn page_state(&self) -> (u64, bool) {
        (self.revision, self.available)
    }

    /// Record the page's request when it names the current state and that state has a report;
    /// false for a request about a state that has passed.
    pub(crate) fn ask_at(&mut self, revision: u64) -> bool {
        let current = self.available && revision == self.revision;
        self.asked |= current;
        current
    }

    /// Take the standing request, once.
    pub(crate) fn take_asked(&mut self) -> bool {
        std::mem::take(&mut self.asked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_is_told_only_of_changes_and_each_names_a_new_revision() {
        let mut offer = ReportOffer::default();
        assert_eq!(offer.page_state(), (0, false));
        assert!(!offer.observe(false));
        assert!(offer.observe(true));
        assert_eq!(offer.page_state(), (1, true));
        assert!(!offer.observe(true));
        assert!(offer.observe(false));
        assert_eq!(offer.page_state(), (2, false));
    }

    #[test]
    fn only_a_request_for_the_current_report_stands_and_it_is_taken_once() {
        let mut offer = ReportOffer::default();
        assert!(!offer.ask_at(0), "nothing to report");
        offer.observe(true);
        assert!(!offer.ask_at(0), "a state that has passed");
        assert!(!offer.take_asked());
        assert!(offer.ask_at(1));
        assert!(offer.ask_at(1), "a repeat changes nothing");
        assert!(offer.take_asked());
        assert!(!offer.take_asked());
    }

    #[test]
    fn a_report_that_goes_away_takes_its_request_along() {
        let mut offer = ReportOffer::default();
        offer.observe(true);
        assert!(offer.ask_at(1));
        offer.observe(false);
        assert!(!offer.take_asked());
        offer.observe(true);
        assert!(!offer.ask_at(1), "the earlier report's revision");
        assert!(offer.ask_at(3));
    }
}
