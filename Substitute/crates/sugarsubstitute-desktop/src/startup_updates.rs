// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Each turn, advance an open update report, tell the Main page whether there is a report to
//! show, and open it when the page asked.
use super::*;

/// Advance an open update report, then read whether `content` has a report to show, tell the
/// page, and open the report when the page asked for it.
pub(super) fn turn(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    target: &EventLoopWindowTarget<NativeAction>,
    proxy: EventLoopProxy<NativeAction>,
    title: &str,
) -> Result<(), DesktopError> {
    super::report::turn(session);
    if session.main.is_none() {
        super::report::open_backend_conflict(session, content, target, proxy.clone(), title)?;
        return super::report::open_startup_error(session, content, target, proxy, title);
    }
    let available =
        content.update_status().as_ref().is_some_and(crate::report::surface::has_update_report);
    let mut asked = false;
    main_operation(session, content, |main| {
        main.observe_update_report(available)?;
        asked = main.take_update_report_request();
        Ok(None)
    })?;
    if asked { super::report::open(session, content, target, proxy, title) } else { Ok(()) }
}
