// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use actual protected files, original Core jobs and split guards at the Token boundary.
use super::super::*;
use super::{
    abi_boundary,
    support::{Harness, TestError},
};
use sugarsubstitute_contracts::{incident::*, lifecycle::*};
use sugarsubstitute_supervision::diagnostic_storage::{fixture::ProtectedCaptureSourceFixture, *};
struct Owned {
    source: ProtectedCaptureSourceFixture,
    storage: DiagnosticStorage,
    operation: DiagnosticStorageOperation,
    token: Token,
}
pub(super) fn owner() -> Result<CoordinatorKey, TestError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([91; 32]),
        epoch: CoordinatorEpoch::new([19; 16])?,
    })
}
pub(super) fn create(harness: &mut Harness) -> Result<ProtectedCaptureSourceFixture, TestError> {
    // SAFETY: CoCreateGuid writes/returns a fresh scalar identifier and owns no handles.
    let id = unsafe { windows::Win32::System::Com::CoCreateGuid() }?;
    let path = std::path::PathBuf::from(format!(
        "D:/SugarSubstitute-Rust/artifacts/k1/incident-host01/finished-token-{id:?}"
    ));
    harness.run(1, ProtectedCaptureSourceFixture::required_reservation_bytes(), move |context| {
        Ok(ProtectedCaptureSourceFixture::create(&context, owner()?, path)?)
    })
}
fn install(
    harness: &mut Harness,
    mut source: ProtectedCaptureSourceFixture,
    mode: &'static str,
) -> Result<(Owned, usize), TestError> {
    let client = harness.client.clone();
    harness.run(2, 8 * 1024 * 1024, move |context| {
        let mut storage = DiagnosticStorage::new(context.memory().claim(8 * 1024 * 1024)?)?;
        let operation = storage.begin_operation(
            LogicalStoreOperationKey::new(owner()?, IncidentOperationId::new(1)?),
            &context,
        )?;
        let mut generation = None;
        storage.prepare_capture_read(source.root_mut(), &mut generation, &operation, &context)?;
        let (cpp, rust) = storage.reserve_finished_owner(
            generation.as_mut().ok_or("no actual generation")?,
            &operation,
            &context,
        )?;
        let mut cpp = Some(cpp);
        let mut rust = Some(rust);
        let mut token = None;
        Token::install(
            &mut token,
            abi_boundary::api(),
            &mut generation,
            &mut cpp,
            &mut rust,
            &context,
        )
        .map_err(|e| format!("install {e:?}"))?;
        let mut token = token.ok_or("no actual token")?;
        token.encode_deadline(&client).map_err(|e| format!("counter {e:?}"))?;
        let key = IncidentKey::new(IncidentSubject::Supervisor(owner()?));
        let opened = token.open(key, &BuildIdentity::new(mode)?, None, &context);
        let fields = token.fields.as_mut().ok_or("no Fields")?;
        let raw = fields.state.token;
        match mode {
            "invalid" => {
                assert_eq!(opened, Err(Issue::Invalid));
                assert!(fields.cpp.is_some());
                assert!(fields.phase == Phase::Quarantined);
            }
            "uncertain" => {
                assert_eq!(opened, Ok(7));
                assert!(fields.cpp.is_some());
                assert!(fields.phase == Phase::Issued);
                assert_eq!(fields.state.token, 0);
            }
            "noowner" => {
                assert_eq!(opened, Ok(7));
                assert!(fields.cpp.is_none());
                assert!(fields.phase == Phase::Retired);
            }
            _ => {
                assert_eq!(opened, Ok(0));
                let mut buffer = [0u8; 16];
                assert_eq!(token.read(0, &mut buffer, &context), Err(Issue::Invalid));
                let fields = token.fields.as_ref().ok_or("no Fields")?;
                assert!(fields.phase == Phase::Quarantined);
                assert!(fields.cpp.is_some());
                assert_eq!(fields.state.token, raw);
                assert_eq!(token.read(0, &mut buffer, &context), Err(Issue::Quarantined));
                assert_eq!(abi_boundary::counts(raw), (1, 0));
            }
        }
        Ok((Owned { source, storage, operation, token }, raw))
    })
}
#[test]
fn foreign_output_quarantines_read_and_retire_before_another_native_effect() -> Result<(), TestError>
{
    let mut harness = Harness::new()?;
    let source = create(&mut harness)?;
    let (mut owned, raw) = install(&mut harness, source, "foreign")?;
    let client = harness.client.clone();
    owned = harness.run(3, 32 * 1024, move |context| {
        assert_eq!(owned.token.retire_cpp(&client, &context), Err(Issue::Quarantined));
        assert_eq!(abi_boundary::counts(raw), (1, 0));
        Ok(owned)
    })?;
    harness.join();
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    assert!(owned.token.fields.as_ref().is_some_and(|fields| fields.cpp.is_some()));
    drop(owned);
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    Ok(())
}
#[test]
fn invalid_open_retirement_does_not_return_cpp_credit() -> Result<(), TestError> {
    let mut harness = Harness::new()?;
    let source = create(&mut harness)?;
    let (owned, _) = install(&mut harness, source, "invalid")?;
    harness.join();
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    drop(owned);
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    Ok(())
}
#[test]
fn oversized_read_retirement_retains_original_token_and_cpp_guard() -> Result<(), TestError> {
    let mut harness = Harness::new()?;
    let source = create(&mut harness)?;
    let (owned, _) = install(&mut harness, source, "oversize")?;
    harness.join();
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    drop(owned);
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    Ok(())
}
#[test]
fn explicit_native_no_owner_allows_source_close_while_rust_data_keeps_original_credit()
-> Result<(), TestError> {
    let mut harness = Harness::new()?;
    let source = create(&mut harness)?;
    let (owned, _) = install(&mut harness, source, "noowner")?;
    let client = harness.client.clone();
    let owned = harness.run(3, 32 * 1024, move |context| {
        let mut owned = owned;
        assert_eq!(owned.token.retire_cpp(&client, &context), Ok(true));
        owned.token.retire_source(&context).map_err(|e| format!("source close {e:?}"))?;
        owned.storage.retire(&owned.operation, &context)?;
        owned.source.retire(&context)?;
        assert!(owned.source.is_retired());
        Ok(owned)
    })?;
    harness.join();
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    drop(owned);
    assert_eq!(harness.main.snapshot().reserved_bytes, 0);
    assert_eq!(harness.source.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn issued_zero_output_without_native_no_owner_proof_keeps_both_roles() -> Result<(), TestError> {
    let mut harness = Harness::new()?;
    let source = create(&mut harness)?;
    let (mut owned, _) = install(&mut harness, source, "uncertain")?;
    let client = harness.client.clone();
    owned = harness.run(3, 32 * 1024, move |context| {
        assert_eq!(owned.token.retire_cpp(&client, &context), Err(Issue::Current));
        assert_eq!(owned.token.retire_source(&context), Err(Issue::Quarantined));
        assert!(owned.token.fields.as_ref().is_some_and(|fields| fields.cpp.is_some()));
        Ok(owned)
    })?;
    harness.join();
    drop(owned);
    assert_eq!(harness.main.snapshot().reserved_bytes, 8 * 1024 * 1024);
    assert_eq!(harness.source.snapshot().reserved_bytes, 1024 * 1024);
    Ok(())
}
