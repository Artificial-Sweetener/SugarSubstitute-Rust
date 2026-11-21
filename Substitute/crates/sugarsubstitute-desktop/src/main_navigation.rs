// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound browser addresses and admit only the exact configured HTTP loopback origin.

use sugarsubstitute_contracts::configuration::{LoopbackHost, LoopbackOrigin};

/// Maximum untrusted native navigation URL length, in UTF-8 bytes.
#[cfg(any(target_os = "windows", test))]
pub(crate) const MAX_URL_BYTES: usize = 4096;

/// Construct the sole initial served root URL from validated domain data.
pub(crate) fn root_url(origin: LoopbackOrigin) -> String {
    let host = match origin.host() {
        LoopbackHost::Ipv4 => "127.0.0.1",
        LoopbackHost::Ipv6 => "[::1]",
        LoopbackHost::Localhost => "localhost",
    };
    format!("http://{host}:{}/", origin.port())
}

/// Bind only the public panel navigation label to the independently authorized loopback root.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn panel_root_url(
    origin: LoopbackOrigin,
    identity: crate::MainIdentity,
) -> Result<String, crate::DesktopError> {
    let session = sugarsubstitute_contracts::panel::PanelSession::for_main(identity)
        .map_err(|cause| crate::DesktopError::new("panel_navigation", cause))?;
    Ok(format!(
        "{}?sc-panel-host=ai.artificialsweetener.sugarsubstitute&sc-panel-session={}&sc-panel-transport=native",
        root_url(origin),
        session.as_str()
    ))
}

/// Validate raw authority before URL parsing can normalize alternate host spelling.
/// Frame and top-document paths remain with the unchanged served frontend.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn admits_url(origin: LoopbackOrigin, value: &str) -> bool {
    if value.len() > MAX_URL_BYTES || value.contains('\0') {
        return false;
    }
    let Some(rest) = value.strip_prefix("http://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let Ok(observed) = LoopbackOrigin::parse(&value[..7 + authority.len()], origin.port().get())
    else {
        return false;
    };
    if observed != origin {
        return false;
    }
    url::Url::parse(value).is_ok_and(|parsed| {
        parsed.scheme() == "http" && parsed.username().is_empty() && parsed.password().is_none()
    })
}

/// The page's own request for a browser permission, as WebView2 reports it.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PermissionRequest {
    /// Reading the clipboard, which the prompt editor's menu Paste needs.
    ClipboardRead,
    /// Any other device or capability: camera, microphone, location, notifications and the rest.
    Other,
}

/// Whether the browser may grant `request` to the page at `uri`.
///
/// Only a clipboard read is ever allowed, only for the application's own page, and only when the
/// person asked for it (a menu Paste); every other permission, and any request from another
/// origin or without a gesture, stays denied as before.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn admits_permission(
    origin: LoopbackOrigin,
    request: PermissionRequest,
    uri: &str,
    user_initiated: bool,
) -> bool {
    request == PermissionRequest::ClipboardRead && user_initiated && admits_url(origin, uri)
}

/// One row of the browser's own context menu, by its unlocalized name.
#[cfg(any(target_os = "windows", test))]
#[derive(Debug)]
pub(crate) struct BrowserMenuRow {
    pub(crate) name: String,
    pub(crate) separator: bool,
}

/// The browser menu rows the application never offers: page inspection and the developer tools
/// submenu. Names are WebView2's unlocalized lower-camel-case English labels.
#[cfg(any(target_os = "windows", test))]
const HIDDEN_BROWSER_MENU_ROWS: [&str; 2] = ["inspectElement", "moreTools"];

/// Indices of the rows to remove, ascending: the hidden rows, and separators left at either end or
/// next to another separator once those rows are gone.
#[cfg(any(target_os = "windows", test))]
pub(crate) fn browser_menu_removals(rows: &[BrowserMenuRow]) -> Vec<usize> {
    let hidden: Vec<bool> = rows
        .iter()
        .map(|row| !row.separator && HIDDEN_BROWSER_MENU_ROWS.contains(&row.name.as_str()))
        .collect();
    if !hidden.contains(&true) {
        return Vec::new();
    }
    let kept: Vec<usize> = (0..rows.len()).filter(|&index| !hidden[index]).collect();
    let mut removed: Vec<usize> = (0..rows.len()).filter(|&index| hidden[index]).collect();
    let mut previous_separator = true;
    for (position, &index) in kept.iter().enumerate() {
        if !rows[index].separator {
            previous_separator = false;
            continue;
        }
        let last = kept[position + 1..].iter().all(|&next| rows[next].separator);
        if previous_separator || last {
            removed.push(index);
        } else {
            previous_separator = true;
        }
    }
    removed.sort_unstable();
    removed
}

#[cfg(test)]
mod browser_menu_tests {
    use super::*;

    fn rows(names: &[&str]) -> Vec<BrowserMenuRow> {
        names
            .iter()
            .map(|name| BrowserMenuRow { name: (*name).into(), separator: *name == "-" })
            .collect()
    }

    #[test]
    fn inspection_rows_go_with_the_separator_they_strand() {
        let menu = rows(&["back", "reload", "-", "moreTools", "-", "inspectElement"]);
        assert_eq!(browser_menu_removals(&menu), vec![2, 3, 4, 5]);
        let editing = rows(&["cut", "copy", "paste", "-", "selectAll", "-", "inspectElement"]);
        assert_eq!(browser_menu_removals(&editing), vec![5, 6]);
    }

    #[test]
    fn a_menu_without_inspection_rows_is_left_alone() {
        assert!(browser_menu_removals(&rows(&["cut", "-", "-", "copy"])).is_empty());
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[test]
    fn only_a_requested_clipboard_read_from_the_application_page_is_admitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let origin = LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?;
        let page = "http://127.0.0.1:8188/?sc-panel-host=x";
        let clipboard = PermissionRequest::ClipboardRead;
        assert!(admits_permission(origin, clipboard, page, true));
        assert!(!admits_permission(origin, clipboard, page, false));
        assert!(!admits_permission(origin, PermissionRequest::Other, page, true));
        assert!(!admits_permission(origin, clipboard, "https://example.com/", true));
        assert!(!admits_permission(origin, clipboard, "http://127.0.0.1:9999/", true));
        Ok(())
    }
}
