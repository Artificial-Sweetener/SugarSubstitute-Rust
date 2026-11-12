// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native installer navigation. Presentation choices never authorize storage or processes.
//! The host executes typed requests and returns their original identities; this owner
//! controls pages, admission, retries and wait feedback without scheduling work.
mod actions;
mod replies;
mod types;
use std::time::Duration;
pub use types::*;

const FRESH: &[Page] = &[
    Page::Language,
    Page::Location,
    Page::Runtime,
    Page::Configuration,
    Page::Folders,
    Page::Progress,
    Page::Complete,
];
const MAINTENANCE: &[Page] =
    &[Page::Language, Page::Location, Page::Maintenance, Page::Progress, Page::Complete];

#[derive(Clone, Copy)]
/// Expected completion semantics for the single outstanding application request.
enum Purpose {
    Locale { advance: bool },
    Location,
    Options(OptionField),
    Runtime,
    Validate,
    Install,
    Maintain,
    Open,
}
/// Original identity retained until a matching terminal result or host shutdown.
struct Pending {
    id: RequestId,
    purpose: Purpose,
}

/// Retained page state for one native installation window.
pub struct SetupFlow {
    page: Page,
    choices: Choices,
    copy_locale: &'static str,
    existing: Option<ExistingInstallation>,
    options: [Vec<Choice>; 3],
    selected: [Option<usize>; 3],
    options_valid: bool,
    mirrors: bool,
    pending: Option<Pending>,
    sequence: u64,
    started: Duration,
    overall: f64,
    status: Status,
    failure: Option<Failure>,
    retry: bool,
    unavailable: bool,
    closing: bool,
    close_requested: bool,
    details: bool,
    revision: u64,
}

impl SetupFlow {
    /// Start at language selection with an application-supplied installation folder.
    /// # Errors
    /// Rejects NUL or oversized editable text before retaining it.
    pub fn new(install_root: String) -> Result<Self, FlowError> {
        valid_text(&install_root)?;
        Ok(Self {
            page: Page::Language,
            choices: Choices {
                locale: "en",
                mode: RuntimeMode::Managed,
                install_root,
                comfy_root: String::new(),
                python: String::new(),
                models: String::new(),
                outputs: String::new(),
                selection_token: None,
            },
            copy_locale: "en",
            existing: None,
            options: std::array::from_fn(|_| Vec::new()),
            selected: [None; 3],
            options_valid: false,
            mirrors: false,
            pending: None,
            sequence: 0,
            started: Duration::ZERO,
            overall: 0.0,
            status: Status::Preparing,
            failure: None,
            retry: false,
            unavailable: false,
            closing: false,
            close_requested: false,
            details: false,
            revision: 0,
        })
    }
    /// Current visible page.
    pub fn page(&self) -> Page {
        self.page
    }
    /// Selected field contents; the installation adapter validates their authority independently.
    pub fn choices(&self) -> &Choices {
        &self.choices
    }
    /// Locale whose labels have been accepted by the application owner.
    pub fn locale(&self) -> &'static str {
        self.copy_locale
    }
    /// Inspection summary used only for presentation and the selected maintenance directory.
    pub fn existing(&self) -> Option<&ExistingInstallation> {
        self.existing.as_ref()
    }
    /// Options retained while a replacement request is pending.
    pub fn options(&self, field: OptionField) -> &[Choice] {
        &self.options[field.index()]
    }
    /// Selected option index within its current list.
    pub fn selected(&self, field: OptionField) -> Option<usize> {
        self.selected[field.index()]
    }
    /// Mirror preference passed through to upstream option discovery.
    pub fn mirrors(&self) -> bool {
        self.mirrors
    }
    /// Whether one application request is outstanding.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    /// Latest operation progress, monotonic within one installation attempt, in percent.
    pub fn overall(&self) -> f64 {
        self.overall
    }
    /// Current localized status key or upstream status text.
    pub fn status(&self) -> &Status {
        &self.status
    }
    /// Display failure without discarding retained field values.
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
    /// Whether the full native output control is revealed.
    pub fn details_open(&self) -> bool {
        self.details
    }
    /// Page revision used by the native host to reset the page viewport on navigation.
    pub fn page_revision(&self) -> u64 {
        self.revision
    }
    /// Successful Open requests ask the host to close; failures leave completion visible.
    pub fn close_requested(&self) -> bool {
        self.close_requested
    }
    /// Native close admission remains unavailable after actual application shutdown begins.
    pub fn is_closing(&self) -> bool {
        self.closing
    }
    /// One-based current step and total steps for the selected installation route.
    pub fn journey(&self) -> (usize, usize) {
        let route = self.route();
        (route.iter().position(|page| *page == self.page).map_or(1, |index| index + 1), route.len())
    }
    /// Localized primary action key; absence hides the action while installing.
    pub fn primary_key(
        &self,
    ) -> Option<sugarsubstitute_contracts::localization::installer::InstallerText> {
        use sugarsubstitute_contracts::localization::installer::InstallerText;
        if self.closing {
            return None;
        }
        if self.retry {
            return Some(InstallerText::Retry);
        }
        match self.page {
            Page::Progress => None,
            Page::Folders => Some(InstallerText::Install),
            Page::Maintenance => Some(InstallerText::Maintain),
            Page::Complete => Some(InstallerText::Open),
            _ => Some(InstallerText::ContinueAction),
        }
    }
    /// Admit an enabled action only when no previous request can still complete it.
    pub fn can_primary(&self) -> bool {
        if self.primary_key().is_none() || self.busy() || self.unavailable || self.close_requested {
            return false;
        }
        if self.page == Page::Configuration && self.choices.mode == RuntimeMode::Managed {
            return self.options_valid;
        }
        true
    }
    /// Keep Back unavailable throughout installation and after completion.
    pub fn can_back(&self) -> bool {
        !self.busy()
            && !self.closing
            && !self.unavailable
            && !matches!(self.page, Page::Language | Page::Progress | Page::Complete)
    }
    /// Give feedback after two minutes without imposing a startup/provisioning deadline.
    pub fn waiting(&self, now: Duration) -> bool {
        !self.unavailable
            && !self.retry
            && !self.closing
            && (self.busy() || self.page == Page::Progress)
            && now.saturating_sub(self.started) >= Duration::from_secs(120)
    }
    /// Reveal/collapse native output without changing installation admission.
    pub fn set_details(&mut self, open: bool) {
        self.details = open;
    }
    /// Report an input-boundary failure while preserving editable choices and navigation.
    /// # Errors
    /// Rejects feedback during work/close or text exceeding the ordinary input bound.
    pub fn input_failed(&mut self, message: String) -> Result<(), FlowError> {
        if !self.can_edit() {
            return Err(FlowError::ActionUnavailable);
        }
        valid_text(&message)?;
        self.failure = Some(Failure::Message(message));
        Ok(())
    }
    /// Seal presentation admission when the host begins actual owned shutdown.
    pub fn closing(&mut self) {
        self.closing = true;
        self.pending = None;
        self.status = Status::Cancelling;
    }
    /// Select maintenance only after a successful existing-installation inspection.
    fn route(&self) -> &'static [Page] {
        if self.existing.is_some() { MAINTENANCE } else { FRESH }
    }
    /// Commit navigation and invalidate the native viewport's page revision.
    fn show(&mut self, page: Page) {
        self.page = page;
        self.pending = None;
        self.revision = self.revision.saturating_add(1);
    }
    /// Issue fresh correlation while retaining the clock across dependent requests.
    fn request(
        &mut self,
        purpose: Purpose,
        operation: Operation,
        now: Duration,
    ) -> Result<Request, FlowError> {
        self.sequence = self.sequence.checked_add(1).ok_or(FlowError::SequenceExhausted)?;
        let id = RequestId(self.sequence);
        if self.pending.is_none() {
            self.started = now;
        }
        self.pending = Some(Pending { id, purpose });
        Ok(Request { id, operation })
    }
    /// Resolve an index only against the retained list that owns it.
    fn selected_choice(&self, field: OptionField) -> Option<&Choice> {
        self.selected[field.index()].and_then(|index| self.options[field.index()].get(index))
    }
}

/// Bound native text shaping and reject embedded string terminators before retention.
fn valid_text(text: &str) -> Result<(), FlowError> {
    if text.contains('\0') || text.encode_utf16().take(4097).count() > 4096 {
        Err(FlowError::InvalidText)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
