// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export canonical reports through a separately admitted finite receiving-file owner.
//!
//! The dispatcher authenticates the Reporter and matches its current selection before
//! invoking this owner. Selection bytes and report bindings grant no filesystem access.
//! Every receiving-file effect needs a genuine execution export admission. Native dumps
//! remain with the committed same-inode reader and its original cleanup authority.
mod archive;
mod error;
mod intent;
mod operation;
#[allow(unsafe_code)]
mod retention;
mod stream;
pub use error::{ReportExportCause, ReportExportCode};
pub use intent::ReportExportIntent;
pub use operation::{ReportExportCustody, ReportExportOperation, ReportExportReceipt};
