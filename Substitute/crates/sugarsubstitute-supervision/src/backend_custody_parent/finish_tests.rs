// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the production Finish owner and receive/write decoder with external boundary facts.
//! Matching protocol data here grants no native peer, authentication or retirement proof.
use super::*;
use crate::backend_custody_parent::state::{correlate_reply, correlate_written, decode_received};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn key() -> Result<BackendCustodyKey, Box<dyn std::error::Error>> {
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
    Ok(BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        BackendCustodySession::new(1)?,
    ))
}
/// Fake only admitted native issue, write/read observations and current admission.
struct Boundary {
    pending: Option<(BackendCustodyFrame, Moment)>,
    receipt: Option<ReceiptFacts>,
    next_nonce: u64,
    write: Option<LifecyclePipeOperationId>,
    read: Option<LifecyclePipeOperationId>,
    admit_calls: usize,
    closed: bool,
}
impl Boundary {
    fn new() -> Self {
        Self {
            pending: None,
            receipt: None,
            next_nonce: 1,
            write: None,
            read: None,
            admit_calls: 0,
            closed: false,
        }
    }
    fn advance(
        &mut self,
        finish: &mut Finish,
        until: Moment,
    ) -> Result<Progress, NativeObservationError> {
        let closed = self.closed;
        let calls = &mut self.admit_calls;
        finish.advance(
            key().map_err(|cause| {
                NativeObservationError::new("test_key", std::io::Error::other(cause.to_string()))
            })?,
            until,
            Exchange {
                next_nonce: &mut self.next_nonce,
                pending: &mut self.pending,
                receipt: &mut self.receipt,
                io_pending: self.write.is_some() || self.read.is_some(),
            },
            || {
                *calls += 1;
                if closed {
                    return Err(NativeObservationError::new(
                        "pipe_current_admission",
                        LifecyclePipeError::Closed,
                    ));
                }
                LifecyclePipeDeadline::new(
                    i64::try_from(until.as_nanos())
                        .map_err(|cause| NativeObservationError::new("test_deadline", cause))?,
                    1_000_000_000,
                )
                .map_err(|cause| NativeObservationError::new("test_counter", cause))
            },
        )
    }
    fn issue(&mut self) -> TestResult {
        assert!(self.pending.is_some());
        self.write = Some(LifecyclePipeOperationId::new(7)?);
        self.read = Some(LifecyclePipeOperationId::new(8)?);
        Ok(())
    }
    fn written(&mut self) -> TestResult {
        correlate_written(
            self.write,
            LifecyclePipeOperationId::new(7)?,
            BACKEND_CUSTODY_FRAME_BYTES as u32,
        )?;
        self.write = None;
        Ok(())
    }
    fn receive(&mut self, value: BackendCustodyFrame, operation: u64) -> TestResult {
        let bytes = value.encode()?;
        let value =
            decode_received(self.read, LifecyclePipeOperationId::new(operation)?, key()?, &bytes)?;
        self.receipt = Some(correlate_reply(self.pending.map(|pending| pending.0), value)?);
        self.pending = None;
        self.read = None;
        Ok(())
    }
    fn response(&self) -> Result<BackendCustodyFrame, Box<dyn std::error::Error>> {
        let original = self.pending.ok_or("Original Finish not queued")?.0;
        Ok(BackendCustodyFrame {
            key: original.key,
            nonce: original.nonce,
            command: BackendCustodyCommand::Receipt(BackendCustodyReceipt::Finished),
        })
    }
}
#[test]
fn received_original_finished_after_checked_write_survives_closed_admission() -> TestResult {
    let until = Moment::from_nanos(1000);
    let mut finish = Finish::new(until);
    let mut boundary = Boundary::new();
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Queued);
    let original = boundary.pending.ok_or("Original absent")?;
    assert_eq!(original.0.nonce, 2);
    assert_eq!(original.1, until);
    boundary.issue()?;
    boundary.written()?;
    boundary.receive(boundary.response()?, 8)?;
    boundary.closed = true;
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Finished);
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Finished);
    assert_eq!(boundary.admit_calls, 1);
    assert_eq!(boundary.next_nonce, 2);
    assert!(boundary.receipt.is_none() && boundary.pending.is_none());
    assert!(size_of::<Finish>() <= size_of::<(Moment, Option<u64>, bool)>());
    assert!(size_of::<Option<Finish>>() <= size_of::<Option<(Moment, Option<u64>, bool)>>());
    Ok(())
}

#[test]
fn closed_without_final_response_retains_original_nonce_and_pending_io() -> TestResult {
    let until = Moment::from_nanos(1000);
    let mut finish = Finish::new(until);
    let mut boundary = Boundary::new();
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Queued);
    boundary.issue()?;
    let original = boundary.pending;
    boundary.closed = true;
    let cause = boundary.advance(&mut finish, until).err().ok_or("Closed source accepted")?;
    assert_eq!(cause.stage(), "pipe_current_admission");
    assert_eq!(boundary.pending, original);
    assert_eq!(boundary.next_nonce, 2);
    assert!(boundary.write.is_some() && boundary.read.is_some() && boundary.receipt.is_none());
    Ok(())
}

#[test]
fn final_response_waits_for_original_full_write_observation() -> TestResult {
    let until = Moment::from_nanos(1000);
    let mut finish = Finish::new(until);
    let mut boundary = Boundary::new();
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Queued);
    boundary.issue()?;
    assert!(
        correlate_written(
            boundary.write,
            LifecyclePipeOperationId::new(9)?,
            BACKEND_CUSTODY_FRAME_BYTES as u32
        )
        .is_err()
    );
    assert!(
        correlate_written(
            boundary.write,
            LifecyclePipeOperationId::new(7)?,
            BACKEND_CUSTODY_FRAME_BYTES as u32 - 1
        )
        .is_err()
    );
    boundary.receive(boundary.response()?, 8)?;
    boundary.closed = true;
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Waiting);
    assert!(boundary.write.is_some() && boundary.receipt.is_some());
    boundary.written()?;
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Finished);
    assert_eq!(boundary.admit_calls, 1);
    Ok(())
}

#[test]
fn wrong_native_operation_key_nonce_kind_and_unsolicited_receipt_remain_rejected() -> TestResult {
    let until = Moment::from_nanos(1000);
    for fault in 0..5 {
        let mut finish = Finish::new(until);
        let mut boundary = Boundary::new();
        assert_eq!(boundary.advance(&mut finish, until)?, Progress::Queued);
        boundary.issue()?;
        let original = boundary.pending;
        let mut response = boundary.response()?;
        match fault {
            1 => {
                response.key =
                    BackendCustodyKey::new(response.key.owner(), BackendCustodySession::new(2)?)
            }
            2 => response.nonce += 1,
            3 => {
                response.command =
                    BackendCustodyCommand::Receipt(BackendCustodyReceipt::Established)
            }
            4 => response.command = original.ok_or("Original absent")?.0.command,
            _ => {}
        }
        assert!(boundary.receive(response, if fault == 0 { 7 } else { 8 }).is_err());
        assert_eq!(boundary.pending, original);
        assert!(boundary.receipt.is_none() && boundary.write.is_some() && boundary.read.is_some());
        assert!(correlate_reply(None, response).is_err());
    }
    Ok(())
}

#[test]
fn renewed_cutoff_is_rejected_before_final_receipt_or_completed_latch_consumption() -> TestResult {
    let until = Moment::from_nanos(1000);
    let changed = Moment::from_nanos(1001);
    let mut finish = Finish::new(until);
    let mut boundary = Boundary::new();
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Queued);
    boundary.issue()?;
    boundary.written()?;
    boundary.receive(boundary.response()?, 8)?;
    boundary.closed = true;
    assert!(boundary.advance(&mut finish, changed).is_err());
    assert!(boundary.receipt.is_some());
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Finished);
    assert!(boundary.advance(&mut finish, changed).is_err());
    assert_eq!(boundary.advance(&mut finish, until)?, Progress::Finished);
    assert_eq!(boundary.next_nonce, 2);
    Ok(())
}

#[path = "finish_tests/first_cause.rs"]
mod first_cause;
