// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise concrete incident and lifecycle owners with deterministic boundary projections.
mod admission;
mod custody;
mod publication;
mod recovery;
mod recovery_outcomes;
mod reporter_queue;
mod reporting;
mod reporting_order;
mod retirement;
mod storage;
mod support;
mod symbols;

mod stored_recovery;

#[path = "reporting/recovery_origin_tests.rs"]
mod recovery_origin;

mod seed_retirement;
