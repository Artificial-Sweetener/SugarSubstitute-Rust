// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Show the file dialog the Main page asked for over the main window, with the loop's shell
//! picker, and tell the page what the person chose.
use super::*;
use crate::file_dialog::{Picker, SelectionKind};
use std::path::{Path, PathBuf};
use sugarsubstitute_contracts::host_file_choice::{
    FileChoiceClientMessage, FileChoiceFailure, FileChoiceHostMessage, FileChoicePurpose,
    FileChoiceRequestName,
};

/// One dialog showing over the main window, holding the loop's picker until it returns.
pub(in crate::startup_runtime) struct ShownChoice {
    picker: Picker,
    request: FileChoiceRequestName,
    folder: PathBuf,
}

/// Settle a showing dialog, then show the next one the page asked for.
pub(super) fn turn(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    proxy: &EventLoopProxy<NativeAction>,
) -> Result<(), DesktopError> {
    settle(session, content)?;
    if session.file_choice.is_some() || session.main.is_none() {
        return Ok(());
    }
    let mut asked = None;
    main_operation(session, content, |main| {
        asked = main.take_file_choice_request();
        Ok(None)
    })?;
    let Some(asked) = asked else {
        return Ok(());
    };
    let request = asked.request.clone();
    let reason = match show(session, asked, proxy) {
        Ok(()) => return Ok(()),
        Err(reason) => reason,
    };
    answer(session, content, FileChoiceHostMessage::Failed { request, reason })
}

/// Queue the dialog for `asked` on the main window, or say why none is shown.
fn show(
    session: &mut Session,
    asked: FileChoiceClientMessage,
    proxy: &EventLoopProxy<NativeAction>,
) -> Result<(), FileChoiceFailure> {
    let folder = PathBuf::from(&asked.folder);
    if !(folder.is_absolute() && folder.is_dir()) {
        return Err(FileChoiceFailure::Unavailable);
    }
    let below = crate::main_file_choice::starting_folder(&folder, &asked.start);
    let start = if below.is_dir() { below } else { folder.clone() };
    let window = session
        .main
        .as_ref()
        .and_then(MainActor::activity_window)
        .ok_or(FileChoiceFailure::Unavailable)?;
    use tao::platform::windows::WindowExtWindows;
    let window = ::windows::Win32::Foundation::HWND(window.hwnd() as *mut _);
    // An open report holds the picker for its own dialogs.
    let picker = session.picker.take().ok_or(FileChoiceFailure::Busy)?;
    let extension = asked.extension.as_str().to_owned();
    let kind = match asked.purpose {
        FileChoicePurpose::Open => SelectionKind::OpenFrom { folder: start, extension },
        FileChoicePurpose::Save { name } => {
            SelectionKind::SaveInto { folder: start, file_name: name, extension }
        }
    };
    let wake = proxy.clone();
    let queued = picker
        .bind(window, move || {
            wake.send_event(NativeAction::Main)
                .map_err(|cause| DesktopError::new("file_choice_wake", cause))
        })
        .and_then(|()| picker.request(kind));
    if let Err(cause) = queued {
        tracing::error!(operation = "show_file_choice", cause = %cause,
            "The file dialog could not be shown");
        if let Err(cause) = picker.release() {
            session.retain(cause);
        }
        session.picker = Some(picker);
        return Err(FileChoiceFailure::Unavailable);
    }
    session.file_choice = Some(ShownChoice { picker, request: asked.request, folder });
    Ok(())
}

/// Close a showing dialog when its window is closing, and answer the page once it returns.
/// The main window is not destroyed while its dialog exists.
fn settle(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
) -> Result<(), DesktopError> {
    let Some(shown) = session.file_choice.take() else {
        return Ok(());
    };
    if (session.closing || session.main.as_ref().is_none_or(MainActor::closing))
        && let Err(cause) = shown.picker.cancel()
    {
        session.retain(cause);
    }
    let Some(result) = shown.picker.take_result() else {
        if let Some(main) = &mut session.main {
            main.hold_for_owned_window(true);
        }
        session.file_choice = Some(shown);
        return Ok(());
    };
    let ShownChoice { picker, request, folder } = shown;
    if let Err(cause) = picker.release() {
        session.retain(cause);
    }
    session.picker = Some(picker);
    answer(session, content, outcome(request, &folder, result))
}

/// What the page is told about a returned dialog.
fn outcome(
    request: FileChoiceRequestName,
    folder: &Path,
    result: Result<Option<PathBuf>, Box<dyn std::error::Error>>,
) -> FileChoiceHostMessage {
    match result {
        Ok(Some(chosen)) => {
            let (name, relative) = crate::main_file_choice::place(folder, &chosen);
            FileChoiceHostMessage::Chosen { request, name, relative }
        }
        Ok(None) => FileChoiceHostMessage::Dismissed { request },
        Err(cause) => {
            tracing::error!(operation = "show_file_choice", cause = %cause,
                "The file dialog failed");
            FileChoiceHostMessage::Failed { request, reason: FileChoiceFailure::Unavailable }
        }
    }
}

/// Tell the page; a main window that is gone hears nothing.
fn answer(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    message: FileChoiceHostMessage,
) -> Result<(), DesktopError> {
    if session.main.is_none() {
        return Ok(());
    }
    main_operation(session, content, |main| main.answer_file_choice(&message).map(|()| None))
}
