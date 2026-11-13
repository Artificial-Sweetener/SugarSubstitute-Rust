// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Withhold a release after repeated bad trials before any of its bytes are downloaded.
use super::*;
use sugarsubstitute_contracts::{installation::PreparationIntent, update::Component};

/// Whether this installation withholds an authenticated automatic application offer.
///
/// Run on an admitted blocking storage job, after the offer is authenticated and before its
/// package is downloaded or reopened. It takes only the shared record read and never enables a
/// selection namespace. Repair requests, updater offers and installations without a selection
/// are never withheld; preparation enforces the same withholding when it reserves a generation.
/// # Errors
/// `LockTimeout` or `LockCancelled` when a publication holds the record past the bounded wait,
/// the deadline or cancellation of `context`, and `Storage` or `Record` for damaged state.
pub fn release_withheld(
    installation: &Path,
    offer: &crate::setup_storage::AuthenticatedOffer,
    trust: &crate::setup_storage::TrustStore,
    context: &JobContext,
) -> Result<bool, SelectionFailure> {
    let request = offer.authorization(trust).request;
    if request.intent != PreparationIntent::Automatic || request.component != Component::Application
    {
        return Ok(false);
    }
    let Some(store) = SelectionStore::existing(installation, context)? else {
        return Ok(false);
    };
    let Some(record) = store.load(context)? else {
        return Ok(false);
    };
    Ok(record.withheld_release(offer.offer().offer().sequence(), store.strike_clock().now()?))
}
