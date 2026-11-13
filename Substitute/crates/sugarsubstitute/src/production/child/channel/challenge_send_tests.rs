// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual shared-send guard without constructing native capabilities.
use super::super::super::requests::RequestError;
use super::{
    tests::{processed, registered},
    *,
};
use sugarsubstitute_contracts::lifecycle::{LifecycleRejection, RequestId};

#[test]
fn independent_request_header_cannot_bypass_native_callback_on_shared_send()
-> Result<(), Box<dyn Error>> {
    let binding = registered()?;
    let mut requests = ClientRequests::new(&binding)?;
    let independent =
        requests.prepare(Moment::from_nanos(100), CounterDeadline::new(1234, 100)?)?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let id = UiChallengeId::new(41)?;
    assert_eq!(
        ui.validate_response(independent, Moment::from_nanos(100), id),
        Err(ChallengeError::Order)
    );
    processed(&mut ui, &binding)?;
    assert_eq!(
        ui.validate_response(independent, Moment::from_nanos(100), id),
        Err(ChallengeError::Identity)
    );
    let PreparedUiResponse { header: actual, original, id: actual_id } =
        ui.response(&mut requests, Moment::from_nanos(0))?.ok_or("No response")?;
    assert_ne!(actual, independent);
    assert_eq!(
        ui.validate_response(independent, original, actual_id),
        Err(ChallengeError::Identity)
    );
    ui.validate_response(actual, original, actual_id)?;
    Ok(())
}

#[test]
fn shared_send_requires_exact_callback_prepared_header_cutoff_and_challenge()
-> Result<(), Box<dyn Error>> {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let mut requests = ClientRequests::new(&binding)?;
    processed(&mut ui, &binding)?;
    let PreparedUiResponse { header: actual, original, id } =
        ui.response(&mut requests, Moment::from_nanos(0))?.ok_or("No response")?;
    for changed in [
        WireHeader { request: Some(RequestId::new(25)?), ..actual },
        WireHeader { deadline_qpc: actual.deadline_qpc + 1, ..actual },
        WireHeader { connection: Some(ConnectionId::new(20)?), ..actual },
        WireHeader { request: None, ..actual },
    ] {
        assert_eq!(ui.validate_response(changed, original, id), Err(ChallengeError::Identity));
    }
    assert_eq!(
        ui.validate_response(actual, Moment::from_nanos(101), id),
        Err(ChallengeError::Identity)
    );
    assert_eq!(
        ui.validate_response(actual, original, UiChallengeId::new(42)?),
        Err(ChallengeError::Identity)
    );
    ui.validate_response(actual, original, id)?;
    requests.admitted(actual)?;
    ui.admitted(actual)?;
    assert_eq!(ui.validate_response(actual, original, id), Err(ChallengeError::Identity));
    ui.close();
    assert_eq!(ui.validate_response(actual, original, id), Err(ChallengeError::Closed));
    Ok(())
}

#[test]
fn rejected_response_retains_original_exchange_and_received_request_evidence()
-> Result<(), Box<dyn Error>> {
    let binding = registered()?;
    let mut ui = UiChallengeExchange::new(&binding)?;
    let mut requests = ClientRequests::new(&binding)?;
    processed(&mut ui, &binding)?;
    let PreparedUiResponse { header: actual, original, .. } =
        ui.response(&mut requests, Moment::from_nanos(0))?.ok_or("No response")?;
    requests.admitted(actual)?;
    ui.admitted(actual)?;
    requests.received(actual, Moment::from_nanos(99))?;
    let cause = ui
        .acknowledged(
            &mut requests,
            actual,
            ReplyOutcome::Rejected(LifecycleRejection::StaleIdentity),
        )
        .err()
        .ok_or("Rejected response accepted")?;
    assert_eq!(cause.downcast_ref::<ChallengeError>(), Some(&ChallengeError::Rejected));
    assert!(ui.matches_reply(actual));
    assert_eq!(ui.deadline(), None);
    assert_eq!(
        requests.outbound(actual, original, Moment::from_nanos(0)),
        Err(RequestError::Order)
    );
    Ok(())
}
