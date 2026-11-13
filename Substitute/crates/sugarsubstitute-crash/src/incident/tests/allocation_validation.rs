// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve genuine callback sources and their actual original control custody.
use super::super::allocation::{ValidationFailure, validation};
use super::{
    publication_fixture::{admitted, setup},
    *,
};
use sugarsubstitute_supervision::diagnostic_storage::*;
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;

fn source<'a, T: Error + 'static>(error: &'a (dyn Error + 'static)) -> Option<&'a T> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(cause) = error.downcast_ref::<T>() {
            return Some(cause);
        }
        current = error.source();
    }
    None
}

#[test]
fn callback_parser_cause_and_last_alias_keep_original_charge_after_native_retirement() -> TestResult
{
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
        let error = validation(&mut decode, || {
            decode_summary(b"{\n\"schema_version\":2,").map(|_| ()).map_err(|cause| {
                ValidationFailure::store(DiagnosticRecoveryValidationCode::InvalidJournal, cause)
            })
        })
        .err()
        .ok_or("Malformed callback record accepted")?;
        let parser = source::<serde_json::Error>(&error).ok_or("Original parser cause absent")?;
        assert!(parser.is_eof());
        assert_eq!(parser.line(), 2);
        assert!(parser.column() > 0);
        let first = error.clone();
        let last = first.clone();
        drop(decode);
        assert!(repository.storage.recovery_decode(&repository.operation, &context).is_err());
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        drop(repository);
        drop(error);
        drop(first);
        assert!(context.memory().claim(1).is_err());
        assert!(source::<serde_json::Error>(&last).is_some_and(serde_json::Error::is_eof));
        drop(last);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn callback_contract_cause_keeps_exact_type_and_original_control_custody() -> TestResult {
    admitted(|context| {
        let (_, mut repository) = setup(&context)?;
        let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
        let error = validation(&mut decode, || {
            IncidentRevision::new(u64::MAX).and_then(IncidentRevision::next).map(|_| ()).map_err(
                |cause| {
                    ValidationFailure::facts(
                        DiagnosticRecoveryValidationCode::ArithmeticOverflow,
                        cause,
                    )
                },
            )
        })
        .err()
        .ok_or("Revision overflow accepted")?;
        assert_eq!(error.code(), DiagnosticRecoveryValidationCode::ArithmeticOverflow);
        assert!(matches!(source::<IncidentError>(&error), Some(IncidentError::RevisionExhausted)));
        drop(decode);
        repository.retire(&context)?;
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn callback_storage_cause_retains_actual_closed_admission_failure() -> TestResult {
    admitted(|context| {
        let (_, mut repository) = setup(&context)?;
        let collection = repository.storage.recovery_collection(&repository.operation, &context)?;
        let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
        let error = validation(&mut decode, || {
            repository
                .storage
                .recovery_collection(&repository.operation, &context)
                .map(|_| ())
                .map_err(|cause| {
                    ValidationFailure::storage(
                        DiagnosticRecoveryValidationCode::InvalidIdentity,
                        cause,
                    )
                })
        })
        .err()
        .ok_or("Occupied collection destination admitted")?;
        let native =
            source::<DiagnosticStorageError>(&error).ok_or("Actual storage cause absent")?;
        assert_eq!(native.code(), DiagnosticStorageFailureCode::Capacity);
        drop(collection);
        drop(decode);
        repository.retire(&context)?;
        drop(repository);
        assert!(context.memory().claim(1).is_err());
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}

#[test]
fn same_decode_second_callback_is_rejected_before_user_parser_runs() -> TestResult {
    admitted(|context| {
        let (_, mut repository) = setup(&context)?;
        let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
        validation(&mut decode, || Ok(()))?;
        let mut parsed = false;
        let error = validation(&mut decode, || {
            parsed = true;
            Ok(())
        })
        .err()
        .ok_or("Consumed validation controls admitted again")?;
        assert!(!parsed);
        let native =
            source::<DiagnosticStorageError>(&error).ok_or("Inline denial source absent")?;
        assert_eq!(native.code(), DiagnosticStorageFailureCode::Capacity);
        drop(error);
        drop(decode);
        repository.retire(&context)?;
        Ok(())
    })
}

#[test]
fn flattened_native_sources_preserve_all_categories_exact_types_and_original_custody() -> TestResult
{
    admitted(|context| {
        for code in [
            DiagnosticRecoveryValidationCode::InvalidSchema,
            DiagnosticRecoveryValidationCode::InvalidJournal,
            DiagnosticRecoveryValidationCode::LedgerMismatch,
            DiagnosticRecoveryValidationCode::ArithmeticOverflow,
            DiagnosticRecoveryValidationCode::InvalidIdentity,
        ] {
            for wrapped in [false, true] {
                let (_, mut repository) = setup(&context)?;
                let collection =
                    repository.storage.recovery_collection(&repository.operation, &context)?;
                let mut decode =
                    repository.storage.recovery_decode(&repository.operation, &context)?;
                let error = validation(&mut decode, || {
                    repository
                        .storage
                        .recovery_collection(&repository.operation, &context)
                        .map(|_| ())
                        .map_err(|cause| {
                            if wrapped {
                                ValidationFailure::store(code, IncidentStoreError::Storage(cause))
                            } else {
                                ValidationFailure::storage(code, cause)
                            }
                        })
                })
                .err()
                .ok_or("Occupied destination admitted")?;
                assert_eq!(error.code(), code);
                assert_eq!(
                    source::<DiagnosticStorageError>(&error).ok_or("Native leaf lost")?.code(),
                    DiagnosticStorageFailureCode::Capacity
                );
                assert_eq!(source::<IncidentStoreError>(&error).is_some(), wrapped);
                drop(collection);
                drop(decode);
                repository.retire(&context)?;
                drop(repository);
                assert!(context.memory().claim(1).is_err());
                drop(error);
                assert_eq!(
                    context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
                    DIAGNOSTIC_STORAGE_BYTES
                );
            }
        }
        Ok(())
    })
}
#[test]
fn flattened_small_store_leaves_preserve_original_variants_and_existing_validation_code()
-> TestResult {
    admitted(|context| {
        let leaves = [
            IncidentStoreError::RecordBound,
            IncidentStoreError::UnsupportedSchema(31),
            IncidentStoreError::RecoveryConflict,
            IncidentStoreError::Capacity,
            IncidentStoreError::ChangedDuplicate,
            IncidentStoreError::StaleRevision,
            IncidentStoreError::RetainedPayloadRequiresDisposition,
            IncidentStoreError::MissingPayload,
            IncidentStoreError::WrongDigest,
        ];
        for leaf in leaves {
            let expected = format!("{leaf:?}");
            let (_, mut repository) = setup(&context)?;
            let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
            let error = validation(&mut decode, || {
                Err::<(), _>(ValidationFailure::store(
                    DiagnosticRecoveryValidationCode::InvalidJournal,
                    leaf,
                ))
            })
            .err()
            .ok_or("Leaf lost")?;
            assert_eq!(
                format!(
                    "{:?}",
                    source::<IncidentStoreError>(&error).ok_or("Original store source absent")?
                ),
                expected
            );
            drop(decode);
            repository.retire(&context)?;
            drop(repository);
            drop(error);
        }
        let (_, mut repository) = setup(&context)?;
        let mut decode = repository.storage.recovery_decode(&repository.operation, &context)?;
        let original = DiagnosticRecoveryValidationError::rejected(
            DiagnosticRecoveryValidationCode::LedgerMismatch,
        );
        let error = validation(&mut decode, || Err::<(), _>(ValidationFailure::from(original)))
            .err()
            .ok_or("Existing error lost")?;
        assert_eq!(error.code(), DiagnosticRecoveryValidationCode::LedgerMismatch);
        assert!(source::<DiagnosticRecoveryValidationError>(&error).is_some());
        drop(decode);
        repository.retire(&context)?;
        drop(repository);
        drop(error);
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
