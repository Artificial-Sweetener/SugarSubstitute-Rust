// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit and retain actual bounded modal selection buffers.
use super::*;
pub(in super::super) struct Outcome {
    pub(in super::super) selection: Option<SaveSelection>,
    pub(in super::super) update: Option<ReportSurfaceUpdate>,
    pub(in super::super) failure: Option<ReportSurfaceError>,
    pub(in super::super) retirement: Option<UnretiredDialog>,
}
pub(in super::super) fn select<T: ReportSessionContent>(
    parent: HWND,
    request: Request,
    content_kind: ExportSelection,
    labels: &ReportMessages,
    lease: &MemoryLease,
    content: &mut T,
) -> Result<Outcome, ReportSurfaceError> {
    let claim = lease
        .claim(
            (size_of::<Hook>()
                + size_of::<SaveSelection>()
                + (SaveSelection::MAX_UNITS + 1) * 2
                + 4096) as u64,
        )
        .map_err(|cause| ReportSurfaceError::new("report_picker_credit", cause))?;
    let title = Text::new(labels.save_title, lease)?;
    let save = Text::new(labels.save, lease)?;
    let cancel_label = Text::new(labels.cancel, lease)?;
    let (filter_label, pattern, extension, default) = match content_kind {
        ExportSelection::TextOnly => (labels.text_filter, "*.txt", w!("txt"), "report.txt"),
        ExportSelection::TextAndNativeDump => {
            (labels.archive_filter, "*.zip", w!("zip"), "report.zip")
        }
    };
    let filter_text = Text::new(filter_label, lease)?;
    let filter_claim = lease
        .claim(((filter_text.units.len() + 16) * 2) as u64)
        .map_err(|cause| ReportSurfaceError::new("report_picker_filter_credit", cause))?;
    let mut filter = Vec::new();
    filter
        .try_reserve_exact(filter_text.units.len() + 16)
        .map_err(|cause| ReportSurfaceError::new("report_picker_filter_allocate", cause))?;
    if filter.capacity() > filter_text.units.len() + 16 {
        return Err(ReportSurfaceError::rejected("report_picker_filter_capacity"));
    }
    filter.extend_from_slice(&filter_text.units);
    filter.extend(pattern.encode_utf16());
    filter.extend([0, 0]);
    let mut destination = Vec::new();
    destination
        .try_reserve_exact(SaveSelection::MAX_UNITS + 1)
        .map_err(|cause| ReportSurfaceError::new("report_picker_allocate", cause))?;
    if destination.capacity() > SaveSelection::MAX_UNITS + 1 {
        return Err(ReportSurfaceError::rejected("report_picker_capacity"));
    }
    destination.resize(SaveSelection::MAX_UNITS + 1, 0);
    for (slot, unit) in destination.iter_mut().zip(default.encode_utf16()) {
        *slot = unit;
    }
    let mut state = Box::new(Hook {
        context: (content as *mut T).cast(),
        poller: Some(poll::<T>),
        active: false,
        initialized: false,
        installed: false,
        destroyed: false,
        timer: 0,
        update: None,
        failure: None,
        save,
        cancel_label,
    });
    let options = Box::new(OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: parent,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(destination.as_mut_ptr()),
        nMaxFile: destination.len() as u32,
        lpstrTitle: PCWSTR(title.units.as_ptr()),
        lpstrDefExt: extension,
        Flags: OFN_EXPLORER
            | OFN_ENABLEHOOK
            | OFN_NOCHANGEDIR
            | OFN_OVERWRITEPROMPT
            | OFN_PATHMUSTEXIST
            | OFN_NOTESTFILECREATE
            | OFN_DONTADDTORECENT,
        lCustData: LPARAM((&mut *state as *mut Hook) as isize),
        lpfnHook: Some(hook),
        ..Default::default()
    });
    let mut dialog = Dialog {
        hook: state,
        options,
        _title: title,
        _filter: filter,
        destination,
        _filter_claim: filter_claim,
        claim,
    };
    // SAFETY: All buffers and callback state remain owned for the entire synchronous native modal call.
    let selected = unsafe { GetSaveFileNameW(&mut *dialog.options) }.as_bool();
    // SAFETY: Capture native extended error immediately; zero means user cancellation.
    let native_error = unsafe { CommDlgExtendedError() };
    dialog.hook.poller = None;
    dialog.hook.context = std::ptr::null_mut();
    if !dialog.hook.retired() {
        let cause =
            dialog.hook.failure.take().unwrap_or_else(|| {
                ReportSurfaceError::rejected("report_picker_uncertain_retirement")
            });
        // Transfer actual unresolved native custody before propagating its original cause.
        return Ok(Outcome {
            selection: None,
            update: None,
            failure: Some(cause),
            retirement: Some(UnretiredDialog::new(dialog)),
        });
    }
    if let Some(cause) = dialog.hook.failure.take() {
        return Err(cause);
    }
    let update = dialog.hook.update.take();
    if update.is_some() {
        return Ok(Outcome { selection: None, update, failure: None, retirement: None });
    }
    if !selected {
        if native_error.0 != 0 {
            return Err(ReportSurfaceError::new(
                "report_picker_native",
                std::io::Error::from_raw_os_error(native_error.0 as i32),
            ));
        }
        return Ok(Outcome { selection: None, update: None, failure: None, retirement: None });
    }
    let length = dialog
        .destination
        .iter()
        .position(|unit| *unit == 0)
        .filter(|length| *length > 0 && *length <= SaveSelection::MAX_UNITS)
        .ok_or_else(|| ReportSurfaceError::rejected("report_picker_destination"))?;
    dialog.destination.truncate(length);
    Ok(Outcome {
        selection: Some(SaveSelection {
            request: request.id,
            binding: request.binding,
            destination: dialog.destination,
            content: content_kind,
            overwrite_consented: true,
            _claim: dialog.claim,
        }),
        update: None,
        failure: None,
        retirement: None,
    })
}
