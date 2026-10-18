// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe causal source/client fences on actual admitted jobs without starting native workers.

use super::*;
use crate::{
    entry_preparation::{
        EntryPreparationInputError,
        staging_support::{self, Prepared},
    },
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec},
    task::{Stop, Work},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{execution::*, lifecycle::NamespaceId};

mod support;
use support::{Fixture, ForeignPorts, Lease};
mod causal;
mod retained;

mod cache;

mod raw_cause;
mod recovery;
