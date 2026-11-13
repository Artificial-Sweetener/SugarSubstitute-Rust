// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feed the Main caption the application's update, close-save and workflow facts each turn,
//! and carry out what its update peers ask for.
use super::*;
use crate::main_updates::{Notice, UpdateFeed, UpdateIntent};

/// Advance an open update report, then read this turn's facts from `content` and show them in
/// the Main caption. The previous close's losses are taken only once the window can show them.
pub(super) fn turn(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
) -> Result<(), DesktopError> {
    super::report::turn(session);
    let Some(main) = &session.main else { return Ok(()) };
    let feed = UpdateFeed::read(
        content.update_status().as_ref(),
        content.pack_update_failure().is_some(),
        content.close_save_progress(),
        content.workflow_activity(),
    );
    let mut notices = Vec::new();
    if main.shows_messages() {
        notices.extend(content.take_update_notice().map(Notice::update));
        notices.extend(content.take_unsaved_notice().map(|notice| Notice::not_saved(&notice)));
    }
    let words = content.locale_messages();
    main_operation(session, content, |main| {
        main.observe_updates(feed, words, notices).map(|()| None)
    })
}

/// Carry out a click on update caption peer `id`.
pub(super) fn activate(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    id: u16,
    target: &EventLoopWindowTarget<NativeAction>,
    proxy: EventLoopProxy<NativeAction>,
    title: &str,
) -> Result<(), DesktopError> {
    let mut intent = None;
    main_operation(session, content, |main| {
        intent = main.activate_update(id)?;
        Ok(None)
    })?;
    match intent {
        Some(UpdateIntent::RestartToApply) => content
            .request_restart_to_apply()
            .map_err(|cause| DesktopError::boxed("restart_to_apply", cause)),
        Some(UpdateIntent::OpenChangelog(url)) => {
            // The release notes are optional: a link the shell cannot open is logged only.
            if let Err(cause) = crate::windows_open_link::open(&url) {
                tracing::warn!(operation = "open_changelog", cause = %cause,
                    "The release notes could not be opened");
            }
            Ok(())
        }
        Some(UpdateIntent::OpenReport) => {
            super::report::open(session, content, target, proxy, title)
        }
        None => Ok(()),
    }
}
