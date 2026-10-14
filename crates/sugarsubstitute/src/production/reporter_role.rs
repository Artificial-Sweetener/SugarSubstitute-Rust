// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the independently enrolled Reporter process and its actual authenticated diagnostic channel.
#[path = "reporter_role/channel.rs"]
mod channel;
#[path = "reporter_role/parent.rs"]
pub(in crate::production) mod parent;

#[path = "reporter_role/client.rs"]
pub(in crate::production) mod client;

#[path = "reporter_role/view.rs"]
pub(in crate::production) mod view;

#[path = "reporter_role/options.rs"]
pub(in crate::production) mod options;

#[path = "reporter_role/source.rs"]
mod source;

#[path = "reporter_role/service.rs"]
mod service;

#[path = "reporter_role/output.rs"]
mod output;

#[path = "reporter_role/runtime.rs"]
pub(in crate::production) mod runtime;

#[path = "reporter_role/parent_source.rs"]
pub(in crate::production) mod parent_source;
