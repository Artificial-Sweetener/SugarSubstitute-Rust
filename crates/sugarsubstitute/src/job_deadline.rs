// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Name the deadline of storage-backed work that ends on its observed outcome.

use sugarsubstitute_contracts::execution::Moment;

/// Deadline for work whose storage callees demand a finite admitted deadline although the work
/// itself ends on its result, an observed failure or cancellation.
///
/// It lies a century past the execution clock's epoch, so elapsed time never reaches it, yet
/// stays representable in every conversion the storage callees apply to a deadline. Shutdown cancels the work through its execution scope,
/// and the storage callees observe that cancellation before each effect, so a slow disk or host
/// delays the work instead of failing the canvas or the workspace.
pub(crate) const OPEN_ENDED: Moment = Moment::from_nanos(100 * 365 * 24 * 3_600 * 1_000_000_000);
