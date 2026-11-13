// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only the external charged guard; exercise actual routing/encoding ownership.
use super::{super::*, support::*};
use std::{cell::Cell, rc::Rc};
use sugarsubstitute_contracts::incident::*;
struct Loan {
    bytes: Vec<u8>,
    retired: Rc<Cell<usize>>,
}
impl Drop for Loan {
    fn drop(&mut self) {
        self.retired.set(self.retired.get() + 1)
    }
}
#[test]
fn rejected_origin_and_malformed_frame_return_original_input_loan() -> TestResult {
    let retired = Rc::new(Cell::new(0));
    let bytes = encoded(storage(1, DiagnosticCommand::Load)?)?;
    let pointer = bytes.as_ptr();
    let loan = Loan { bytes, retired: retired.clone() };
    let wrong =
        DiagnosticKey::new(owner(1)?, DiagnosticRole::Storage, DiagnosticSessionId::new(9)?);
    let (error, mut loan) =
        routing::receive(wrong, key(DiagnosticRole::Storage)?, loan, |g| &g.bytes)
            .err()
            .ok_or("rejection")?;
    assert_eq!(error, DiagnosticWireError::Identity);
    assert_eq!(retired.get(), 0);
    assert_eq!(loan.bytes.as_ptr(), pointer);
    loan.bytes[0] = 0;
    let (error, loan) =
        routing::receive(key(DiagnosticRole::Storage)?, key(DiagnosticRole::Storage)?, loan, |g| {
            &g.bytes
        })
        .err()
        .ok_or("rejection")?;
    assert_eq!(error, DiagnosticWireError::Schema);
    assert_eq!(retired.get(), 0);
    drop(loan);
    assert_eq!(retired.get(), 1);
    Ok(())
}
#[test]
fn codec_rejection_preserves_original_output_guard_and_bytes() -> TestResult {
    let retired = Rc::new(Cell::new(0));
    let loan = Loan { bytes: vec![0xa5; 8], retired: retired.clone() };
    let pointer = loan.bytes.as_ptr();
    let (error, loan) =
        routing::prepare(loan, storage(1, DiagnosticCommand::Load)?, |g| &mut g.bytes)
            .err()
            .ok_or("rejection")?;
    assert_eq!(error, DiagnosticWireError::Capacity);
    assert_eq!(loan.bytes.as_ptr(), pointer);
    assert_eq!(loan.bytes, [0xa5; 8]);
    assert_eq!(retired.get(), 0);
    drop(loan);
    assert_eq!(retired.get(), 1);
    Ok(())
}
#[test]
fn accepted_guard_stays_owned_until_consumer_disposes_it() -> TestResult {
    let retired = Rc::new(Cell::new(0));
    let loan = Loan { bytes: vec![0; MAX_FRAME_BYTES], retired: retired.clone() };
    let pointer = loan.bytes.as_ptr();
    let (mut loan, length) =
        routing::prepare(loan, storage(1, DiagnosticCommand::Load)?, |g| &mut g.bytes)
            .map_err(|(e, _)| e)?;
    loan.bytes.truncate(length);
    let loan =
        routing::receive(key(DiagnosticRole::Storage)?, key(DiagnosticRole::Storage)?, loan, |g| {
            &g.bytes
        })
        .map_err(|(e, _)| e)?;
    assert_eq!(retired.get(), 0);
    assert_eq!(loan.bytes.as_ptr(), pointer);
    assert_eq!(decode(&loan.bytes)?.request.get(), 1);
    drop(loan);
    assert_eq!(retired.get(), 1);
    Ok(())
}
