// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the concrete owner through its public lease/admission contracts.
#[path = "supervision/admission.rs"]
mod admission;
#[path = "supervision/enrollment.rs"]
mod enrollment;
#[path = "supervision/leases.rs"]
mod leases;
#[path = "supervision/outage.rs"]
mod outage;
#[path = "supervision/outage_capacity.rs"]
mod outage_capacity;
#[path = "supervision/resource_storage.rs"]
mod resource_storage;
#[path = "supervision/run_settlement.rs"]
mod run_settlement;
#[path = "supervision/support.rs"]
mod support;
