// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compile the executable-private entry owners through their actual package inputs.
#![cfg(windows)]
#[path = "../src/job_sequence.rs"]
mod job_sequence;
#[path = "../src/production.rs"]
mod production;
